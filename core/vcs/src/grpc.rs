//! The gRPC services that the server calls: `echo/v1`, and the
//! services of `source/v1` when vcs has a store.

mod deadline;
mod echo;
mod reads;
mod repos;

use std::net::SocketAddr;

use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
use jjforge_proto::prost_types;
use jjforge_proto::source::v1::AmbiguousRevision;
use jjforge_proto::source::v1::RepoRef;
use jjforge_proto::source::v1::diff_service_server::DiffServiceServer;
use jjforge_proto::source::v1::file_service_server::FileServiceServer;
use jjforge_proto::source::v1::history_service_server::HistoryServiceServer;
use jjforge_proto::source::v1::repo_service_server::RepoServiceServer;

use jjforge_store::server::Repo;
use jjforge_store::server::RepoError;
use prost::Message as _;
use tonic::Request;
use tonic::Status;

use self::deadline::Deadline;
use crate::auth::Access;
use crate::gate::Gate;
use crate::gate::Refused;
use crate::metrics::Meter;
use crate::telemetry::TraceGrpc;

/// The telemetry tests trace and meter a call to it.
#[cfg(test)]
pub(crate) use echo::Echo;

/// The longest a call's handler runs before vcs answers `DEADLINE_EXCEEDED`,
/// as ADR 0006 decides.
const DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

/// Serves gRPC on `addr` until the process gets Ctrl-C.
pub(crate) async fn serve(addr: SocketAddr, gate: Option<Gate>) -> anyhow::Result<()> {
    tracing::info!(%addr, "gRPC listening");
    // The trace layer wraps every service added here, so none needs code of
    // its own to be traced.
    tonic::transport::Server::builder()
        .layer(Meter)
        .layer(TraceGrpc)
        .layer(Deadline(DEADLINE))
        .add_service(EchoServiceServer::new(echo::Echo))
        .add_optional_service(
            gate.clone()
                .map(|gate| HistoryServiceServer::new(reads::Service(gate))),
        )
        .add_optional_service(
            gate.clone()
                .map(|gate| FileServiceServer::new(reads::Service(gate))),
        )
        .add_optional_service(
            gate.clone()
                .map(|gate| DiffServiceServer::new(reads::Service(gate))),
        )
        .add_optional_service(gate.map(|gate| RepoServiceServer::new(repos::Service(gate))))
        .serve_with_shutdown(addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

/// Opens the repository that a call names, for the principal in its
/// `authorization` metadata, through the gate.
fn open<T>(
    gate: &Gate,
    request: &Request<T>,
    repo: Option<&RepoRef>,
    access: Access,
) -> Result<Repo, Status> {
    let repo = repo.ok_or_else(|| Status::invalid_argument("the request names no repository"))?;
    let authorization = request
        .metadata()
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    let (repo, _) = gate.open(authorization, &repo.org_id, &repo.repo_id, access)?;
    Ok(repo)
}

impl From<Refused> for Status {
    fn from(err: Refused) -> Self {
        match err {
            Refused::Unauthenticated(_) => Status::unauthenticated(err.to_string()),
            Refused::Forbidden => Status::permission_denied(err.to_string()),
            Refused::Repo(err) => status(err),
        }
    }
}

/// `google.rpc.Status`, which carries the details of a gRPC error in the
/// `grpc-status-details-bin` trailer, as gRPC's richer error model decides.
#[derive(Clone, PartialEq, prost::Message)]
struct RichStatus {
    #[prost(int32, tag = "1")]
    code: i32,
    #[prost(string, tag = "2")]
    message: String,
    #[prost(message, repeated, tag = "3")]
    details: Vec<prost_types::Any>,
}

/// A store error as a gRPC status. An ambiguous revision names its
/// candidates in the message, and carries them as an `AmbiguousRevision`
/// in the details, so the server reads them without parsing text.
#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err hands over the error"
)]
fn status(err: RepoError) -> Status {
    match &err {
        RepoError::NotFound(_) => Status::not_found(err.to_string()),
        RepoError::Invalid(_) => Status::invalid_argument(err.to_string()),
        RepoError::Ambiguous(rev, candidates) => {
            let ids: Vec<_> = candidates
                .iter()
                .map(|change| change.commit_id.as_str())
                .collect();
            let message = format!("{err}: {}", ids.join(", "));
            let detail = AmbiguousRevision {
                rev: rev.clone(),
                candidates: candidates.clone(),
            };
            let details = RichStatus {
                code: tonic::Code::FailedPrecondition.into(),
                message: message.clone(),
                details: vec![prost_types::Any {
                    type_url: "type.googleapis.com/source.v1.AmbiguousRevision".to_owned(),
                    value: detail.encode_to_vec(),
                }],
            };
            Status::with_details(
                tonic::Code::FailedPrecondition,
                message,
                details.encode_to_vec().into(),
            )
        }
        RepoError::Internal(_) => {
            tracing::error!(error = %err, "store failed");
            Status::internal("the store failed")
        }
    }
}

#[cfg(test)]
mod tests {
    use jjforge_proto::source::v1::Change;

    use super::*;

    #[test]
    fn an_ambiguous_revision_carries_its_candidates_in_the_details() {
        let candidate = |commit_id: &str| Change {
            change_id: "kmzvqrts".to_owned(),
            commit_id: commit_id.to_owned(),
            ..Change::default()
        };
        let err = RepoError::Ambiguous(
            "kmzv".to_owned(),
            vec![candidate("5e1c"), candidate("a4b3")],
        );

        let status = status(err);

        let details = RichStatus::decode(status.details()).unwrap();
        let [detail] = details.details.as_slice() else {
            panic!("{details:?}");
        };
        let ambiguous = AmbiguousRevision::decode(detail.value.as_slice()).unwrap();
        assert_eq!(status.code(), tonic::Code::FailedPrecondition);
        assert_eq!(status.message(), "revision kmzv is ambiguous: 5e1c, a4b3");
        assert_eq!(details.code, i32::from(tonic::Code::FailedPrecondition));
        assert_eq!(
            detail.type_url,
            "type.googleapis.com/source.v1.AmbiguousRevision"
        );
        assert_eq!(ambiguous.rev, "kmzv");
        assert_eq!(ambiguous.candidates.len(), 2);
    }
}

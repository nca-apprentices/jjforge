//! The gRPC services that the server calls: `echo/v1`, and the
//! services of `source/v1` when vcs has a store.

mod echo;
mod reads;
mod repos;

use std::net::SocketAddr;

use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
use jjforge_proto::source::v1::RepoRef;
use jjforge_proto::source::v1::diff_service_server::DiffServiceServer;
use jjforge_proto::source::v1::file_service_server::FileServiceServer;
use jjforge_proto::source::v1::history_service_server::HistoryServiceServer;
use jjforge_proto::source::v1::repo_service_server::RepoServiceServer;

use jjforge_store::server::Repo;
use jjforge_store::server::RepoError;
use tonic::Request;
use tonic::Status;

use crate::auth::Access;
use crate::gate::Gate;
use crate::gate::Refused;
use crate::metrics::Meter;
use crate::telemetry::TraceGrpc;

/// The telemetry tests trace and meter a call to it.
#[cfg(test)]
pub(crate) use echo::Echo;

/// Serves gRPC on `addr` until the process gets Ctrl-C.
pub(crate) async fn serve(addr: SocketAddr, gate: Option<Gate>) -> anyhow::Result<()> {
    tracing::info!(%addr, "gRPC listening");
    // The trace layer wraps every service added here, so none needs code of
    // its own to be traced.
    tonic::transport::Server::builder()
        .layer(Meter)
        .layer(TraceGrpc)
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

/// A store error as a gRPC status. An ambiguous revision names its
/// candidates in the message.
#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err hands over the error"
)]
fn status(err: RepoError) -> Status {
    match &err {
        RepoError::NotFound(_) => Status::not_found(err.to_string()),
        RepoError::Invalid(_) => Status::invalid_argument(err.to_string()),
        RepoError::Ambiguous(_, candidates) => {
            Status::failed_precondition(format!("{err}: {}", candidates.join(", ")))
        }
        RepoError::Internal(_) => {
            tracing::error!(error = %err, "store failed");
            Status::internal("the store failed")
        }
    }
}

//! `RepoService` of `source/v1`, which the server calls to create and delete
//! a repository's storage, as #25 and #28 decide. The call carries the
//! caller's session, and vcs verifies it as `sync/v1` does. Creating and
//! deleting need the admin scope, which a session for pushing lacks.

use jjforge_proto::source::v1::CreateRepoRequest;
use jjforge_proto::source::v1::CreateRepoResponse;
use jjforge_proto::source::v1::DeleteRepoRequest;
use jjforge_proto::source::v1::DeleteRepoResponse;
use jjforge_proto::source::v1::GetHeadRequest;
use jjforge_proto::source::v1::GetHeadResponse;
use jjforge_proto::source::v1::RepoRef;
use jjforge_proto::source::v1::repo_service_server::RepoService;
use jjforge_store::server::Repo;
use tonic::Request;
use tonic::Response;
use tonic::Status;

use crate::auth::Access;
use crate::gate::Gate;

pub(crate) struct Service(pub(crate) Gate);

impl Service {
    fn open<T>(
        &self,
        request: &Request<T>,
        repo: Option<&RepoRef>,
        access: Access,
    ) -> Result<Repo, Status> {
        super::open(&self.0, request, repo, access)
    }
}

#[tonic::async_trait]
impl RepoService for Service {
    async fn create_repo(
        &self,
        request: Request<CreateRepoRequest>,
    ) -> Result<Response<CreateRepoResponse>, Status> {
        let repo = self.open(&request, request.get_ref().repo.as_ref(), Access::Admin)?;
        repo.create().await.map_err(super::status)?;
        Ok(Response::new(CreateRepoResponse {}))
    }

    async fn delete_repo(
        &self,
        request: Request<DeleteRepoRequest>,
    ) -> Result<Response<DeleteRepoResponse>, Status> {
        let repo = self.open(&request, request.get_ref().repo.as_ref(), Access::Admin)?;
        repo.delete().await.map_err(super::status)?;
        Ok(Response::new(DeleteRepoResponse {}))
    }

    async fn get_head(
        &self,
        request: Request<GetHeadRequest>,
    ) -> Result<Response<GetHeadResponse>, Status> {
        let repo = self.open(&request, request.get_ref().repo.as_ref(), Access::Read)?;
        let head = repo.head().await.map_err(super::status)?;
        Ok(Response::new(GetHeadResponse {
            operation_id: head.to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use jjforge_proto::source::v1::repo_service_server::RepoServiceServer;
    use jjforge_store::server::Repos;
    use tower::Layer as _;
    use tower::Service as _;
    use url::Url;

    use super::*;
    use crate::auth::SessionKey;
    use crate::auth::Verifier;
    use crate::auth::token_with;
    use crate::telemetry::attribute;
    use crate::telemetry::finished_spans;

    /// A gRPC request body: one uncompressed, length-prefixed message.
    fn frame(message: &impl prost::Message) -> Vec<u8> {
        let bytes = message.encode_to_vec();
        let mut frame = vec![0];
        frame.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_be_bytes());
        frame.extend(bytes);
        frame
    }

    #[tokio::test]
    async fn every_call_is_traceable_with_its_org_repo_and_principal() {
        const TRACE: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
        let key = SessionKey::random().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let repos = Repos::open(&Url::parse("memory:///").unwrap(), cache.path()).unwrap();
        let verifier = Verifier::new(vec![], key.clone());
        let mut service = crate::telemetry::TraceGrpc
            .layer(RepoServiceServer::new(Service(Gate::new(repos, verifier))));
        let token = token_with(&key, "sync:admin:acme sync:read:acme");
        let repo = || {
            Some(RepoRef {
                org_id: "acme".to_owned(),
                repo_id: "demo".to_owned(),
            })
        };
        let calls = [
            ("CreateRepo", frame(&CreateRepoRequest { repo: repo() })),
            ("GetHead", frame(&GetHeadRequest { repo: repo() })),
            ("DeleteRepo", frame(&DeleteRepoRequest { repo: repo() })),
        ];

        for (method, body) in calls {
            let request = http::Request::post(format!("/source.v1.RepoService/{method}"))
                .header("content-type", "application/grpc")
                .header("traceparent", format!("00-{TRACE}-00f067aa0ba902b7-01"))
                .header("authorization", format!("Bearer {token}"))
                .body(tonic::body::Body::new(axum::body::Body::from(body)))
                .unwrap();

            let (_, spans) = finished_spans(async { service.call(request).await }).await;

            assert!(
                spans
                    .iter()
                    .all(|span| span.span_context.trace_id().to_string() == TRACE),
                "{method}"
            );
            for (key, value) in [
                ("jjforge.org.id", "acme"),
                ("jjforge.repo.id", "demo"),
                ("jjforge.principal.id", "ada"),
            ] {
                assert_eq!(attribute(&spans, key).as_deref(), Some(value), "{method}");
            }
        }
    }
}

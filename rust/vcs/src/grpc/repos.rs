//! `RepoService` of `source/v1`, which the server calls to create and delete
//! a repository's storage, as #25 and #28 decide. The call carries the
//! caller's token, and vcs verifies it as `sync/v1` does. Creating and
//! deleting need the admin scope, which a token for pushing lacks.

use jjforge_proto::source::v1::CreateRepoRequest;
use jjforge_proto::source::v1::CreateRepoResponse;
use jjforge_proto::source::v1::DeleteRepoRequest;
use jjforge_proto::source::v1::DeleteRepoResponse;
use jjforge_proto::source::v1::GetHeadRequest;
use jjforge_proto::source::v1::GetHeadResponse;
use jjforge_proto::source::v1::RepoRef;
use jjforge_proto::source::v1::repo_service_server::RepoService;
use jjforge_store::server::Repo;
use jjforge_store::server::RepoError;
use tonic::Request;
use tonic::Response;
use tonic::Status;
use tracing::field::Empty;

use crate::auth::Access;
use crate::gate::Gate;
use crate::gate::Refused;

pub(crate) struct Service(pub(crate) Gate);

impl Service {
    async fn open<T>(
        &self,
        request: &Request<T>,
        repo: Option<&RepoRef>,
        access: Access,
    ) -> Result<Repo, Status> {
        let repo =
            repo.ok_or_else(|| Status::invalid_argument("the request names no repository"))?;
        let authorization = request
            .metadata()
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        let (repo, _) = self
            .0
            .open(authorization, &repo.org_id, &repo.repo_id, access)
            .await?;
        Ok(repo)
    }
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err hands over the error"
)]
fn status(err: RepoError) -> Status {
    match err {
        RepoError::NotFound(_) => Status::not_found(err.to_string()),
        RepoError::Invalid(_) => Status::invalid_argument(err.to_string()),
        RepoError::Internal(_) => {
            tracing::error!(error = %err, "store failed");
            Status::internal("the store failed")
        }
    }
}

/// Each call opens a span of its own under the gRPC call's, because only
/// a span that declares the gate's fields keeps them.
#[tonic::async_trait]
impl RepoService for Service {
    #[tracing::instrument(skip_all, fields(
        jjforge.org.id = Empty,
        jjforge.repo.id = Empty,
        jjforge.principal.id = Empty,
    ))]
    async fn create_repo(
        &self,
        request: Request<CreateRepoRequest>,
    ) -> Result<Response<CreateRepoResponse>, Status> {
        let repo = self
            .open(&request, request.get_ref().repo.as_ref(), Access::Admin)
            .await?;
        repo.create().await.map_err(status)?;
        Ok(Response::new(CreateRepoResponse {}))
    }

    #[tracing::instrument(skip_all, fields(
        jjforge.org.id = Empty,
        jjforge.repo.id = Empty,
        jjforge.principal.id = Empty,
    ))]
    async fn delete_repo(
        &self,
        request: Request<DeleteRepoRequest>,
    ) -> Result<Response<DeleteRepoResponse>, Status> {
        let repo = self
            .open(&request, request.get_ref().repo.as_ref(), Access::Admin)
            .await?;
        repo.delete().await.map_err(status)?;
        Ok(Response::new(DeleteRepoResponse {}))
    }

    #[tracing::instrument(skip_all, fields(
        jjforge.org.id = Empty,
        jjforge.repo.id = Empty,
        jjforge.principal.id = Empty,
    ))]
    async fn get_head(
        &self,
        request: Request<GetHeadRequest>,
    ) -> Result<Response<GetHeadResponse>, Status> {
        let repo = self
            .open(&request, request.get_ref().repo.as_ref(), Access::Read)
            .await?;
        let head = repo.head().await.map_err(status)?;
        Ok(Response::new(GetHeadResponse {
            operation_id: head.to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use jjforge_proto::source::v1::repo_service_server::RepoServiceServer;
    use jjforge_store::server::Repos;
    use tonic_tracing_opentelemetry::middleware::server::OtelGrpcLayer;
    use tower::Layer as _;
    use tower::Service as _;
    use url::Url;

    use super::*;
    use crate::auth::Issuer;
    use crate::auth::TestKey;
    use crate::auth::Verifier;
    use crate::auth::issuer_with;
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
    async fn a_call_names_its_org_repo_and_principal() {
        let key = TestKey::new();
        let issuer = issuer_with("acme", &key).await;
        let cache = tempfile::tempdir().unwrap();
        let repos = Repos::open(&Url::parse("memory:///").unwrap(), cache.path()).unwrap();
        repos.get("acme", "demo").unwrap().create().await.unwrap();
        let verifier = Verifier::new(vec![Issuer::new("acme", issuer.clone())]).unwrap();
        let mut service = OtelGrpcLayer::default()
            .layer(RepoServiceServer::new(Service(Gate::new(repos, verifier))));
        let body = frame(&GetHeadRequest {
            repo: Some(RepoRef {
                org_id: "acme".to_owned(),
                repo_id: "demo".to_owned(),
            }),
        });
        let request = http::Request::post("/source.v1.RepoService/GetHead")
            .header("content-type", "application/grpc")
            .header(
                "authorization",
                format!("Bearer {}", token_with(&issuer, &key, "sync:read:acme")),
            )
            .body(tonic::body::Body::new(axum::body::Body::from(body)))
            .unwrap();

        let (_, spans) = finished_spans(service.call(request)).await;

        assert_eq!(attribute(&spans, "jjforge.org.id").as_deref(), Some("acme"));
        assert_eq!(
            attribute(&spans, "jjforge.repo.id").as_deref(),
            Some("demo")
        );
        assert_eq!(
            attribute(&spans, "jjforge.principal.id").as_deref(),
            Some("ada")
        );
    }
}

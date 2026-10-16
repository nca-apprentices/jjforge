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

#[tonic::async_trait]
impl RepoService for Service {
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

//! The one way a request reaches a repository, over gRPC or `sync/v1`: its
//! token must grant the access it asks for. A principal without any access
//! to the organization learns nothing, not even that the repository exists,
//! as #27 decides.

use std::sync::Arc;

use jjforge_store::server::Author;
use jjforge_store::server::Repo;
use jjforge_store::server::RepoError;
use jjforge_store::server::Repos;

use crate::auth::Access;
use crate::auth::AuthError;
use crate::auth::Verifier;

/// The repositories behind the token check.
#[derive(Clone)]
pub(crate) struct Gate {
    repos: Arc<Repos>,
    verifier: Arc<Verifier>,
}

/// Why a request didn't reach a repository. Each transport answers it with
/// its own status.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Refused {
    #[error(transparent)]
    Unauthenticated(#[from] AuthError),
    /// The principal may read the organization but not do this.
    #[error("the token doesn't allow this")]
    Forbidden,
    /// The repository doesn't exist, or the principal may not know it does.
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl Gate {
    pub(crate) fn new(repos: Repos, verifier: Verifier) -> Self {
        Self {
            repos: Arc::new(repos),
            verifier: Arc::new(verifier),
        }
    }

    /// Verifies the bearer token in `authorization`, checks that it grants
    /// `access` to the repositories of `org`, and opens `org/repo`. The
    /// current span records whom the request was for, as ADR 0005 decides.
    ///
    /// # Errors
    ///
    /// Fails when the token doesn't verify, when it lacks `access`, or when
    /// the repository's IDs are invalid.
    pub(crate) async fn open(
        &self,
        authorization: Option<&str>,
        org: &str,
        repo: &str,
        access: Access,
    ) -> Result<(Repo, Author), Refused> {
        let principal = self.verifier.verify(authorization).await?;
        if !principal.may(Access::Read, org) {
            return Err(RepoError::NotFound(format!("repository {org}/{repo}")).into());
        }
        if !principal.may(access, org) {
            return Err(Refused::Forbidden);
        }

        let span = tracing::Span::current();
        span.record("jjforge.org.id", org);
        span.record("jjforge.repo.id", repo);
        span.record("jjforge.principal.id", principal.subject.as_str());
        Ok((self.repos.get(org, repo)?, principal.into()))
    }
}

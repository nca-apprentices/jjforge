//! The one way a request reaches a repository, over gRPC or `sync/v1`: its
//! principal must have the access it asks for. A principal without any access
//! to the organization learns nothing, not even that the repository exists,
//! as #27 decides.

use std::sync::Arc;

use jjforge_store::server::Author;
use jjforge_store::server::Repo;
use jjforge_store::server::RepoError;
use jjforge_store::server::Repos;

use crate::auth::Access;
use crate::auth::AuthError;
use crate::auth::Principals;

/// The repositories behind the principal check.
#[derive(Clone)]
pub(crate) struct Gate {
    repos: Arc<Repos>,
    principals: Arc<Principals>,
}

/// Why a request didn't reach a repository. Each transport answers it with
/// its own status.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Refused {
    #[error(transparent)]
    Unauthenticated(#[from] AuthError),
    /// The principal may read the organization but not do this.
    #[error("the principal may not do this")]
    Forbidden,
    /// The repository doesn't exist, or the principal may not know it does.
    #[error(transparent)]
    Repo(#[from] RepoError),
}

impl Gate {
    pub(crate) fn new(repos: Repos, principals: Principals) -> Self {
        Self {
            repos: Arc::new(repos),
            principals: Arc::new(principals),
        }
    }

    /// Finds the principal in `authorization`, checks that it has `access`
    /// to the repositories of `org`, and opens `org/repo`. The current span
    /// records whom the request was for, as ADR 0005 decides, and why it
    /// refused it, so it must declare `jjforge.org.id`, `jjforge.repo.id`,
    /// `jjforge.principal.id`, and `jjforge.refusal`.
    ///
    /// # Errors
    ///
    /// Fails when vcs doesn't know the principal, when it lacks `access`, or
    /// when the repository's IDs are invalid.
    pub(crate) fn open(
        &self,
        authorization: Option<&str>,
        org: &str,
        repo: &str,
        access: Access,
    ) -> Result<(Repo, Author), Refused> {
        // The span names whom the request was for even when it's refused,
        // so a trace shows who tried.
        let span = tracing::Span::current();
        span.record("jjforge.org.id", org);
        span.record("jjforge.repo.id", repo);
        let principal = match self.principals.verify(authorization) {
            Ok(principal) => principal,
            Err(err) => {
                span.record("jjforge.refusal", "unauthenticated");
                return Err(err.into());
            }
        };
        span.record("jjforge.principal.id", principal.subject.as_str());

        if !principal.may(Access::Read, org) {
            span.record("jjforge.refusal", "hidden");
            return Err(RepoError::NotFound(format!("repository {org}/{repo}")).into());
        }
        if !principal.may(access, org) {
            span.record("jjforge.refusal", "forbidden");
            return Err(Refused::Forbidden);
        }
        Ok((self.repos.get(org, repo)?, principal.into()))
    }
}

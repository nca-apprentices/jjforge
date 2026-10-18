//! Who sent a request and what it may do. vcs authenticates no one until
//! sign-in exists, as #6 plans: a request names its principal as
//! `Authorization: Bearer {name}`, and the file that `VCS_PRINCIPALS` names
//! grants each name its scopes. Without the file, vcs knows no principal and
//! refuses every request, so a deployed vcs stays closed.
//!
//! Each line of the file holds a name and its scopes, separated by spaces.
//! A scope is `sync:read:{org}`, `sync:write:{org}`, or `sync:admin:{org}`:
//!
//! ```text
//! # The owner of acme.
//! acme_owner sync:write:acme sync:admin:acme
//! ```

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;

use jjforge_store::server::Author;

/// What a request asks of a repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    Read,
    Write,
    /// Creating and deleting a repository, which only the server asks for
    /// on behalf of an owner, as #25 and #28 decide.
    Admin,
}

/// The principal a request names and what it may do.
#[derive(Clone, Debug)]
pub(crate) struct Principal {
    pub(crate) subject: String,
    scopes: HashSet<String>,
}

impl Principal {
    /// Whether the principal has `access` to the repositories of `org`.
    /// Write implies read, and admin implies neither.
    pub(crate) fn may(&self, access: Access, org: &str) -> bool {
        let write = self.scopes.contains(&format!("sync:write:{org}"));
        match access {
            Access::Read => write || self.scopes.contains(&format!("sync:read:{org}")),
            Access::Write => write,
            Access::Admin => self.scopes.contains(&format!("sync:admin:{org}")),
        }
    }
}

impl From<Principal> for Author {
    fn from(principal: Principal) -> Self {
        Self {
            subject: principal.subject,
            delegation: vec![],
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AuthError {
    #[error("the request names no principal")]
    Missing,
    /// The reason goes to the log only, so a client learns nothing about
    /// which principals vcs knows.
    #[error("the principal is unknown")]
    Unknown(String),
}

/// The principals vcs knows, by name.
#[derive(Debug, Default)]
pub(crate) struct Principals(HashMap<String, Principal>);

impl FromStr for Principals {
    type Err = String;

    /// Reads the lines of a principals file. A line that starts with `#`
    /// and an empty line hold no principal.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut principals = HashMap::new();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut words = line.split_whitespace();
            let subject = words.next().unwrap_or_default().to_owned();
            let principal = Principal {
                subject: subject.clone(),
                scopes: words.map(str::to_owned).collect(),
            };
            if principals.insert(subject.clone(), principal).is_some() {
                return Err(format!("{subject} appears twice"));
            }
        }
        Ok(Self(principals))
    }
}

impl Principals {
    /// Reads the principals file at `path`.
    ///
    /// # Errors
    ///
    /// Fails when vcs can't read the file or a name appears twice.
    pub(crate) fn read(path: &Path) -> anyhow::Result<Self> {
        std::fs::read_to_string(path)
            .map_err(anyhow::Error::from)
            .and_then(|text| text.parse().map_err(anyhow::Error::msg))
            .map_err(|err| anyhow::anyhow!("{}: {err}", path.display()))
    }

    /// The principal that the bearer name in `authorization` names.
    ///
    /// # Errors
    ///
    /// Fails on a missing or an unknown name.
    pub(crate) fn verify(&self, authorization: Option<&str>) -> Result<Principal, AuthError> {
        // The scheme's name is case-insensitive, as RFC 9110 decides.
        let name = authorization
            .and_then(|value| value.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, name)| name.trim())
            .ok_or(AuthError::Missing)?;
        self.0.get(name).cloned().ok_or_else(|| {
            tracing::info!(principal = name, "principal refused");
            AuthError::Unknown(name.to_owned())
        })
    }
}

/// The principals of the tests, each named for what it may do.
#[cfg(test)]
pub(crate) fn test_principals() -> Principals {
    "reader sync:read:acme
writer sync:write:acme
admin sync:admin:acme sync:read:acme
stranger sync:write:other"
        .parse()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(scope: &str) -> Principal {
        format!("ada {scope}")
            .parse::<Principals>()
            .unwrap()
            .verify(Some("Bearer ada"))
            .unwrap()
    }

    #[test]
    fn only_admin_creates_and_deletes() {
        let writer = principal("sync:write:acme");
        let admin = principal("sync:admin:acme sync:write:acme");

        assert!(!writer.may(Access::Admin, "acme"));
        assert!(admin.may(Access::Admin, "acme"));
        assert!(!admin.may(Access::Admin, "other"));
    }

    #[test]
    fn write_implies_read_of_the_same_org_only() {
        let writer = principal("sync:write:acme");
        let reader = principal("sync:read:acme");

        assert!(writer.may(Access::Read, "acme"));
        assert!(writer.may(Access::Write, "acme"));
        assert!(!writer.may(Access::Read, "other"));
        assert!(reader.may(Access::Read, "acme"));
        assert!(!reader.may(Access::Write, "acme"));
    }

    #[test]
    fn a_request_without_a_bearer_name_is_refused() {
        let principals: Principals = "ada sync:read:acme".parse().unwrap();

        assert!(matches!(principals.verify(None), Err(AuthError::Missing)));
        assert!(matches!(
            principals.verify(Some("Basic ada")),
            Err(AuthError::Missing)
        ));
    }

    #[test]
    fn an_unknown_name_is_refused() {
        let principals: Principals = "ada sync:read:acme".parse().unwrap();

        assert!(matches!(
            principals.verify(Some("Bearer mallory")),
            Err(AuthError::Unknown(_))
        ));
    }

    #[test]
    fn vcs_without_principals_refuses_every_name() {
        assert!(matches!(
            Principals::default().verify(Some("Bearer ada")),
            Err(AuthError::Unknown(_))
        ));
    }

    #[test]
    fn a_file_skips_comments_and_refuses_a_name_twice() {
        let principals: Principals = "# people\n\nada sync:read:acme\n".parse().unwrap();

        assert!(principals.verify(Some("bearer ada")).is_ok());
        assert!("ada\nada\n".parse::<Principals>().is_err());
    }
}

//! Proves who sent a request with OpenSSH certificates, as ADR 0002
//! decides, and reads what the certificate allows.
//!
//! ```text
//! jf ── GET  /sync/v1/challenge ─────────────────────────▶ vcs
//!    ◀─ a nonce that vcs MACs, so it remembers nothing ───
//! jf ── POST /sync/v1/session: certificate, nonce, SSHSIG ▶ vcs
//!    ◀─ a session, at most 15 minutes ────────────────────
//! jf ── any other route, Authorization: Bearer {session} ▶ vcs
//! ```
//!
//! A certificate names its principal in its key ID. Its extension
//! `jjforge-scope@nca-apprentices.dev` grants scopes such as
//! `sync:read:{org}`, `sync:write:{org}`, and `sync:admin:{org}`, separated
//! by spaces, and `jjforge-delegation@nca-apprentices.dev` names the
//! principals that delegated to it, outermost first, separated by commas.

mod session;

use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use jjforge_store::server::Author;
use jjforge_store::server::Signer;
use ssh_key::Certificate;
use ssh_key::Fingerprint;
use ssh_key::HashAlg;
use ssh_key::LineEnding;
use ssh_key::PrivateKey;
use ssh_key::PublicKey;
use ssh_key::SshSig;
use ssh_key::certificate::CertType;

pub(crate) use self::session::Claims;
pub(crate) use self::session::SessionKey;

/// The namespace of the signature over a challenge, so no signature made
/// for anything else, such as a commit, opens a session.
pub(crate) const NAMESPACE: &str = "jjforge-session";

const SCOPE: &str = "jjforge-scope@nca-apprentices.dev";
const DELEGATION: &str = "jjforge-delegation@nca-apprentices.dev";

/// How long a session lasts at most. Revoking a certificate takes effect
/// when its sessions end.
const SESSION_LIFETIME: Duration = Duration::from_mins(15);

/// How long a challenge stays valid.
const CHALLENGE_LIFETIME: Duration = Duration::from_secs(60);

/// What a request asks of a repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    Read,
    Write,
    /// Creating and deleting a repository, which only the server asks for
    /// on behalf of an owner, as #25 and #28 decide.
    Admin,
}

/// The principal of a verified session and what it may do.
#[derive(Debug)]
pub(crate) struct Principal {
    pub(crate) subject: String,
    /// The principals that delegated to this one, outermost first.
    pub(crate) delegation: Vec<String>,
    scopes: HashSet<String>,
    /// The organizations whose authorities signed the certificate, the
    /// only ones its scopes reach.
    orgs: HashSet<String>,
}

impl Principal {
    /// Whether the session grants `access` to the repositories of `org`.
    /// Write implies read, and admin implies neither.
    pub(crate) fn may(&self, access: Access, org: &str) -> bool {
        if !self.orgs.contains(org) {
            return false;
        }
        let write = self.scopes.contains(&format!("sync:write:{org}"));
        match access {
            Access::Read => write || self.scopes.contains(&format!("sync:read:{org}")),
            Access::Write => write,
            Access::Admin => self.scopes.contains(&format!("sync:admin:{org}")),
        }
    }
}

impl From<Claims> for Principal {
    fn from(claims: Claims) -> Self {
        Self {
            subject: claims.sub,
            delegation: claims.delegation,
            scopes: claims.scopes.into_iter().collect(),
            orgs: claims.orgs.into_iter().collect(),
        }
    }
}

impl From<Principal> for Author {
    fn from(principal: Principal) -> Self {
        Self {
            subject: principal.subject,
            delegation: principal.delegation,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AuthError {
    #[error("the request has no session")]
    Missing,
    /// The reason goes to the log only, so a client learns nothing about
    /// how vcs checks a credential.
    #[error("the credential is invalid")]
    Invalid(String),
}

/// A certificate authority that vcs trusts, bound to the one organization
/// whose scopes its certificates grant, written `{org}={path}`, where the
/// file holds the authority's public key in OpenSSH form. One authority can
/// sign for several organizations by appearing once for each.
#[derive(Clone, Debug)]
pub(crate) struct Authority {
    org: String,
    fingerprint: Fingerprint,
}

impl Authority {
    pub(crate) fn new(org: &str, key: &PublicKey) -> Self {
        Self {
            org: org.to_owned(),
            fingerprint: key.fingerprint(HashAlg::Sha256),
        }
    }
}

impl FromStr for Authority {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (org, path) = value
            .split_once('=')
            .filter(|(org, path)| !org.is_empty() && !path.is_empty())
            .ok_or_else(|| format!("{value:?} is not {{org}}={{path}}"))?;
        let path = PathBuf::from(path);
        let key = PublicKey::read_openssh_file(&path)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        Ok(Self::new(org, &key))
    }
}

/// The audience that the server signs for over `SessionService`. The server
/// only ever calls the vcs of its configuration, so no other site can relay
/// its challenge.
pub(crate) const GRPC_AUDIENCE: &str = "grpc:source/v1";

/// What a client signs: the origin it connected to and the nonce, so a
/// challenge that another site relays to it opens nothing here.
pub(crate) fn signed(origin: &str, nonce: &[u8]) -> Vec<u8> {
    [origin.as_bytes(), b"\n", nonce].concat()
}

pub(crate) struct Verifier {
    authorities: Vec<Authority>,
    key: SessionKey,
    /// The origins that clients reach vcs at, such as
    /// `https://jjforge.example.com`. Empty allows loopback origins only.
    origins: Vec<String>,
}

impl Verifier {
    /// Trusts the certificates that `authorities` sign, and seals sessions
    /// with `key`, which every replica shares.
    pub(crate) fn new(authorities: Vec<Authority>, key: SessionKey) -> Self {
        Self {
            authorities,
            key,
            origins: vec![],
        }
    }

    /// Accepts signatures for `origins` only, instead of loopback origins.
    pub(crate) fn with_origins(mut self, origins: Vec<String>) -> Self {
        self.origins = origins;
        self
    }

    fn allows(&self, origin: &str) -> bool {
        if origin == GRPC_AUDIENCE {
            return true;
        }
        if !self.origins.is_empty() {
            return self.origins.iter().any(|allowed| allowed == origin);
        }
        url::Url::parse(origin)
            .is_ok_and(|url| matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
    }

    /// A nonce for a client to sign, valid for `CHALLENGE_LIFETIME`.
    ///
    /// # Errors
    ///
    /// Fails when the system has no randomness to give.
    pub(crate) fn challenge(&self) -> anyhow::Result<Vec<u8>> {
        self.key.challenge(now())
    }

    /// Opens a session for the principal of `certificate`, which signed
    /// `origin` and `nonce` with `signature`, as [`signed`] joins them.
    ///
    /// # Errors
    ///
    /// Fails on an origin that isn't vcs's, on a stale or forged nonce, on a certificate that no trusted
    /// authority signed or that isn't valid now, and on a signature by
    /// another key or for another namespace.
    pub(crate) fn session(
        &self,
        certificate: &str,
        origin: &str,
        nonce: &[u8],
        signature: &str,
    ) -> Result<(String, u64), AuthError> {
        let result = self.open_session(certificate, origin, nonce, signature);
        if let Err(AuthError::Invalid(reason)) = &result {
            tracing::info!(%reason, "session refused");
        }
        result
    }

    fn open_session(
        &self,
        certificate: &str,
        origin: &str,
        nonce: &[u8],
        signature: &str,
    ) -> Result<(String, u64), AuthError> {
        let now = now();
        let invalid = |reason: &str| AuthError::Invalid(reason.to_owned());
        if !self.allows(origin) {
            return Err(AuthError::Invalid(format!(
                "{origin} is not an origin of vcs"
            )));
        }
        if !self.key.check_challenge(nonce, now, CHALLENGE_LIFETIME) {
            return Err(invalid("the challenge is stale or forged"));
        }

        let certificate = Certificate::from_openssh(certificate.trim())
            .map_err(|err| AuthError::Invalid(err.to_string()))?;
        if certificate.cert_type() != CertType::User {
            return Err(invalid("the certificate is not a user certificate"));
        }
        // vcs understands no critical option, so it can honor none.
        if !certificate.critical_options().is_empty() {
            return Err(invalid("the certificate has a critical option"));
        }
        if certificate.key_id().is_empty() {
            return Err(invalid("the certificate names no principal"));
        }
        let orgs: Vec<String> = self
            .authorities
            .iter()
            .filter(|authority| {
                certificate
                    .validate_at(now, [&authority.fingerprint])
                    .is_ok()
            })
            .map(|authority| authority.org.clone())
            .collect();
        if orgs.is_empty() {
            return Err(invalid(
                "no trusted authority signed the certificate, or it isn't valid now",
            ));
        }

        let signature: SshSig = signature
            .parse()
            .map_err(|err: ssh_key::Error| AuthError::Invalid(err.to_string()))?;
        PublicKey::from(certificate.public_key().clone())
            .verify(NAMESPACE, &signed(origin, nonce), &signature)
            .map_err(|err| AuthError::Invalid(err.to_string()))?;

        let extension = |name: &str| certificate.extensions().get(name).cloned();
        let expires = certificate
            .valid_before()
            .min(now + SESSION_LIFETIME.as_secs());
        let claims = Claims {
            sub: certificate.key_id().to_owned(),
            delegation: extension(DELEGATION)
                .map(|chain| {
                    chain
                        .split(',')
                        .filter(|principal| !principal.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            scopes: extension(SCOPE)
                .map(|scopes| scopes.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_default(),
            orgs,
            exp: expires,
        };
        Ok((self.key.seal(&claims), expires))
    }

    /// Verifies the session in an `Authorization: Bearer` header and returns
    /// its principal.
    ///
    /// # Errors
    ///
    /// Fails on a missing, forged, or expired session.
    pub(crate) fn verify(&self, authorization: Option<&str>) -> Result<Principal, AuthError> {
        // The scheme's name is case-insensitive, as RFC 9110 decides.
        let token = authorization
            .and_then(|value| value.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, token)| token.trim())
            .ok_or(AuthError::Missing)?;
        let result = self.key.open(token, now()).map(Principal::from);
        if let Err(AuthError::Invalid(reason)) = &result {
            tracing::info!(%reason, "session refused");
        }
        result
    }
}

/// The namespace of vcs's signature over an operation.
const OPERATION_NAMESPACE: &str = "jjforge-operation";

/// Signs each operation vcs writes with the unencrypted OpenSSH key at
/// `path`, as ADR 0001 decides.
///
/// # Errors
///
/// Fails when the file holds no unencrypted private key.
pub(crate) fn operation_signer(path: &std::path::Path) -> anyhow::Result<Signer> {
    let key = PrivateKey::read_openssh_file(path)
        .map_err(|err| anyhow::anyhow!("{}: {err}", path.display()))?;
    anyhow::ensure!(!key.is_encrypted(), "{} is encrypted", path.display());
    Ok(Arc::new(move |bytes: &[u8]| {
        key.sign(OPERATION_NAMESPACE, HashAlg::Sha512, bytes)
            .and_then(|signature| signature.to_pem(LineEnding::LF))
            .map(String::into_bytes)
            .unwrap_or_default()
    }))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod testing;

#[cfg(test)]
pub(crate) use testing::ORIGIN;
#[cfg(test)]
pub(crate) use testing::TestAuthority;
#[cfg(test)]
pub(crate) use testing::token_with;

#[cfg(test)]
mod tests {
    use super::testing::ORIGIN;
    use super::testing::TestAuthority;
    use super::*;

    fn principal(scope: &str) -> Principal {
        Principal {
            subject: "ada".to_owned(),
            delegation: vec![],
            scopes: scope.split_whitespace().map(str::to_owned).collect(),
            orgs: HashSet::from(["acme".to_owned()]),
        }
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
    fn a_request_without_a_bearer_session_is_refused() {
        let verifier = Verifier::new(vec![], SessionKey::random().unwrap());

        assert!(matches!(verifier.verify(None), Err(AuthError::Missing)));
        assert!(matches!(
            verifier.verify(Some("Basic abc")),
            Err(AuthError::Missing)
        ));
    }

    #[test]
    fn a_signed_challenge_opens_a_session_with_the_certificate_scopes() {
        let authority = TestAuthority::new();
        let verifier = authority.verifier("acme");
        let person = authority.person("ada", "sync:write:acme", "");

        let (token, _) = person.session(&verifier).unwrap();
        let principal = verifier.verify(Some(&format!("Bearer {token}"))).unwrap();

        assert_eq!(principal.subject, "ada");
        assert!(principal.may(Access::Write, "acme"));
        assert!(!principal.may(Access::Admin, "acme"));
    }

    #[test]
    fn a_session_names_the_delegation_chain_outermost_first() {
        let authority = TestAuthority::new();
        let verifier = authority.verifier("acme");
        let agent = authority.person("bot", "sync:read:acme", "ada,job");

        let (token, _) = agent.session(&verifier).unwrap();
        let principal = verifier.verify(Some(&format!("Bearer {token}"))).unwrap();

        assert_eq!(principal.delegation, ["ada", "job"]);
    }

    #[test]
    fn an_authority_grants_nothing_outside_its_organization() {
        let authority = TestAuthority::new();
        let verifier = authority.verifier("other");
        let person = authority.person("ada", "sync:write:acme", "");

        let (token, _) = person.session(&verifier).unwrap();
        let principal = verifier.verify(Some(&format!("Bearer {token}"))).unwrap();

        assert!(!principal.may(Access::Read, "acme"));
    }

    #[test]
    fn a_certificate_from_an_untrusted_authority_opens_no_session() {
        let trusted = TestAuthority::new();
        let stranger = TestAuthority::new();
        let verifier = trusted.verifier("acme");

        let person = stranger.person("mallory", "sync:write:acme", "");

        assert!(matches!(
            person.session(&verifier),
            Err(AuthError::Invalid(_))
        ));
    }

    #[test]
    fn a_signature_by_another_key_opens_no_session() {
        let authority = TestAuthority::new();
        let verifier = authority.verifier("acme");
        let ada = authority.person("ada", "sync:write:acme", "");
        let mallory = authority.person("mallory", "sync:read:acme", "");
        let nonce = verifier.challenge().unwrap();

        let signature = mallory.sign(&signed(ORIGIN, &nonce), NAMESPACE);

        assert!(matches!(
            verifier.session(&ada.certificate(), ORIGIN, &nonce, &signature),
            Err(AuthError::Invalid(_))
        ));
    }

    #[test]
    fn a_signature_for_another_namespace_opens_no_session() {
        let authority = TestAuthority::new();
        let verifier = authority.verifier("acme");
        let ada = authority.person("ada", "sync:write:acme", "");
        let nonce = verifier.challenge().unwrap();

        let signature = ada.sign(&signed(ORIGIN, &nonce), "git");

        assert!(matches!(
            verifier.session(&ada.certificate(), ORIGIN, &nonce, &signature),
            Err(AuthError::Invalid(_))
        ));
    }

    #[test]
    fn a_nonce_that_vcs_did_not_issue_opens_no_session() {
        let authority = TestAuthority::new();
        let verifier = authority.verifier("acme");
        let ada = authority.person("ada", "sync:write:acme", "");
        let nonce = vec![7; 56];

        let signature = ada.sign(&signed(ORIGIN, &nonce), NAMESPACE);

        assert!(matches!(
            verifier.session(&ada.certificate(), ORIGIN, &nonce, &signature),
            Err(AuthError::Invalid(_))
        ));
    }

    // Another forge that trusts the same authority hands ada a challenge of
    // this vcs. Ada signs the origin she connected to, so it opens nothing
    // here.
    #[test]
    fn a_challenge_relayed_by_another_site_opens_no_session() {
        let authority = TestAuthority::new();
        let verifier = authority
            .verifier("acme")
            .with_origins(vec!["https://jjforge.example.com".to_owned()]);
        let ada = authority.person("ada", "sync:write:acme", "");
        let nonce = verifier.challenge().unwrap();
        let evil = "https://evil.example";

        let signature = ada.sign(&signed(evil, &nonce), NAMESPACE);

        for origin in [evil, "https://jjforge.example.com"] {
            assert!(
                matches!(
                    verifier.session(&ada.certificate(), origin, &nonce, &signature),
                    Err(AuthError::Invalid(_))
                ),
                "{origin}"
            );
        }
    }

    #[test]
    fn a_verifier_without_origins_allows_loopback_only() {
        let verifier = TestAuthority::new().verifier("acme");

        assert!(verifier.allows("http://localhost:8081"));
        assert!(verifier.allows("http://127.0.0.1:28081"));
        assert!(verifier.allows(GRPC_AUDIENCE));
        assert!(!verifier.allows("https://jjforge.example.com"));
    }

    #[test]
    fn an_expired_certificate_opens_no_session() {
        let authority = TestAuthority::new();
        let verifier = authority.verifier("acme");
        let ada = authority.expired("ada", "sync:write:acme");

        assert!(matches!(ada.session(&verifier), Err(AuthError::Invalid(_))));
    }

    #[test]
    fn a_session_lasts_no_longer_than_its_certificate() {
        let authority = TestAuthority::new();
        let verifier = authority.verifier("acme");
        let ada = authority.person("ada", "sync:write:acme", "");

        let (_, expires) = ada.session(&verifier).unwrap();

        assert!(expires <= ada.valid_before());
        assert!(expires <= now() + SESSION_LIFETIME.as_secs());
    }
}

//! Authorities, certificates, and sessions for the tests of vcs. Each test
//! generates its keys, so no private key lives in the repository.

#![allow(clippy::unwrap_used, reason = "a panic in a test is the failure")]

use ssh_key::Algorithm;
use ssh_key::Certificate;
use ssh_key::HashAlg;
use ssh_key::LineEnding;
use ssh_key::PrivateKey;
use ssh_key::certificate::Builder;
use ssh_key::certificate::CertType;
use ssh_key::rand_core::OsRng;

use super::Audience;
use super::AuthError;
use super::Authority;
use super::Claims;
use super::DELEGATION;
use super::NAMESPACE;
use super::SCOPE;
use super::SessionKey;
use super::Verifier;
use super::now;
use super::signed;

/// The origin that the tests reach vcs at, a loopback one, which a verifier
/// without origins allows.
pub(crate) const ORIGIN: &str = "http://localhost:8081";

/// A certificate authority that signs for the tests.
pub(crate) struct TestAuthority(PrivateKey);

/// A person's key and the certificate an authority signed for it.
pub(crate) struct TestPerson {
    key: PrivateKey,
    certificate: Certificate,
}

impl TestAuthority {
    pub(crate) fn new() -> Self {
        Self(PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap())
    }

    /// A verifier that trusts this authority for `org`.
    pub(crate) fn verifier(&self, org: &str) -> Verifier {
        Verifier::new(vec![self.authority(org)], SessionKey::random().unwrap())
    }

    /// This authority, trusted for `org`.
    pub(crate) fn authority(&self, org: &str) -> Authority {
        Authority::new(org, self.0.public_key())
    }

    /// A certificate for `name` that holds for an hour, with `scope` and
    /// the delegation chain `delegation`.
    pub(crate) fn person(&self, name: &str, scope: &str, delegation: &str) -> TestPerson {
        let now = now();
        self.sign(name, scope, delegation, now - 60, now + 3_600)
    }

    /// A certificate for `name` that expired a minute ago.
    pub(crate) fn expired(&self, name: &str, scope: &str) -> TestPerson {
        let now = now();
        self.sign(name, scope, "", now - 3_600, now - 60)
    }

    fn sign(
        &self,
        name: &str,
        scope: &str,
        delegation: &str,
        after: u64,
        before: u64,
    ) -> TestPerson {
        let key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519).unwrap();
        let mut builder =
            Builder::new_with_random_nonce(&mut OsRng, key.public_key(), after, before).unwrap();
        builder.cert_type(CertType::User).unwrap();
        builder.key_id(name).unwrap();
        builder.valid_principal(name).unwrap();
        builder.extension(SCOPE, scope).unwrap();
        if !delegation.is_empty() {
            builder.extension(DELEGATION, delegation).unwrap();
        }
        let certificate = builder.sign(&self.0).unwrap();
        TestPerson { key, certificate }
    }
}

impl TestPerson {
    pub(crate) fn certificate(&self) -> String {
        self.certificate.to_openssh().unwrap()
    }

    pub(crate) fn valid_before(&self) -> u64 {
        self.certificate.valid_before()
    }

    /// The SSHSIG over `message`, as `ssh-keygen -Y sign` writes it.
    pub(crate) fn sign(&self, message: &[u8], namespace: &str) -> String {
        self.key
            .sign(namespace, HashAlg::Sha512, message)
            .unwrap()
            .to_pem(LineEnding::LF)
            .unwrap()
    }

    /// Signs a challenge of `verifier` for [`ORIGIN`] and opens a session
    /// with it.
    pub(crate) fn session(&self, verifier: &Verifier) -> Result<(String, u64), AuthError> {
        let nonce = verifier.challenge().unwrap();
        let signature = self.sign(&signed(ORIGIN, &nonce), NAMESPACE);
        verifier.session(
            &self.certificate(),
            Audience::Origin(ORIGIN),
            &nonce,
            &signature,
        )
    }
}

/// A session that `key` sealed for `ada` in `acme`, with `scope`.
pub(crate) fn token_with(key: &SessionKey, scope: &str) -> String {
    key.seal(&Claims {
        sub: "ada".to_owned(),
        delegation: vec![],
        scopes: scope.split_whitespace().map(str::to_owned).collect(),
        orgs: vec!["acme".to_owned()],
        exp: now() + 3_600,
    })
}

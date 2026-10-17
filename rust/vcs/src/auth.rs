//! Verifies the token of a `sync/v1` request offline, as ADR 0002 decides,
//! against the key sets of the issuers vcs trusts, and reads what it allows.
//!
//! A token names its principal in `sub`, carries `jjforge` in `aud`, and
//! grants scopes such as `sync:read:{org}`, `sync:write:{org}`, and
//! `sync:admin:{org}` in `scope`. An agent's token names the principals that delegated to it in
//! nested `act` claims, as RFC 8693 defines them.

use std::collections::HashMap;
use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;
use std::time::Instant;

use jjforge_store::server::Author;
use jsonwebtoken::Algorithm;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::Validation;
use jsonwebtoken::jwk::AlgorithmParameters;
use jsonwebtoken::jwk::JwkSet;
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use url::Url;

/// The audience every token for vcs names.
const AUDIENCE: &str = "jjforge";

/// The shortest time between two reloads of the key sets, so a stream of
/// tokens with unknown keys can't make vcs hammer the issuers.
const RELOAD_EVERY: Duration = Duration::from_secs(10);

/// What a request asks of a repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    Read,
    Write,
    /// Creating and deleting a repository, which only the server asks for
    /// on behalf of an owner, as #25 and #28 decide.
    Admin,
}

/// The principal of a verified token and what it may do.
#[derive(Debug)]
pub(crate) struct Principal {
    pub(crate) subject: String,
    /// The principals that delegated to this one, outermost first.
    pub(crate) delegation: Vec<String>,
    scopes: HashSet<String>,
    /// The organization of the token's issuer, the only one its scopes
    /// reach.
    org: String,
}

impl Principal {
    /// Whether the token grants `access` to the repositories of `org`. Write
    /// implies read, and admin implies neither.
    pub(crate) fn may(&self, access: Access, org: &str) -> bool {
        if org != self.org {
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

#[derive(Debug, thiserror::Error)]
pub(crate) enum AuthError {
    #[error("the request has no bearer token")]
    Missing,
    /// The reason goes to the log only, so a client learns nothing about
    /// how vcs parses a token.
    #[error("the token is invalid")]
    Invalid(String),
}

impl From<Principal> for Author {
    fn from(principal: Principal) -> Self {
        Self {
            subject: principal.subject,
            delegation: principal.delegation,
        }
    }
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    #[serde(default)]
    scope: String,
    act: Option<Actor>,
    // jsonwebtoken 9 skips a time claim it can't parse (GHSA-h395-gr6q-cpjc).
    // Typing both here makes such a token fail to decode.
    #[allow(dead_code, reason = "checked by jsonwebtoken, typed here")]
    exp: u64,
    #[allow(dead_code, reason = "checked by jsonwebtoken, typed here")]
    nbf: Option<u64>,
}

#[derive(Deserialize)]
struct Actor {
    sub: String,
    act: Option<Box<Actor>>,
}

#[derive(Deserialize)]
struct Discovery {
    jwks_uri: String,
}

/// An issuer that vcs trusts, bound to the one organization whose scopes
/// its tokens grant, written `{org}={url}`. Each organization has an
/// identity provider of its own until #6, so a provider can't sign for
/// another organization.
#[derive(Clone, Debug)]
pub(crate) struct Issuer {
    org: String,
    url: Url,
}

impl Issuer {
    pub(crate) fn new(org: &str, url: Url) -> Self {
        Self {
            org: org.to_owned(),
            url,
        }
    }
}

impl FromStr for Issuer {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (org, url) = value
            .split_once('=')
            .filter(|(org, _)| !org.is_empty())
            .ok_or_else(|| format!("{value:?} is not {{org}}={{url}}"))?;
        let url = Url::parse(url).map_err(|err| err.to_string())?;
        Ok(Self::new(org, url))
    }
}

/// A key and the issuer that published it.
struct Key {
    issuer: String,
    org: String,
    decoding: DecodingKey,
    family: Family,
    /// The algorithm the key set names, which RFC 7517 leaves optional.
    algorithm: Option<Algorithm>,
}

/// The kind of a public key, which bounds the algorithms it verifies. A
/// symmetric key in a public key set would let anyone sign, so it has none.
#[derive(Clone, Copy)]
enum Family {
    Rsa,
    EllipticCurve,
    Edwards,
}

impl Family {
    fn of(parameters: &AlgorithmParameters) -> Option<Self> {
        match parameters {
            AlgorithmParameters::RSA(_) => Some(Self::Rsa),
            AlgorithmParameters::EllipticCurve(_) => Some(Self::EllipticCurve),
            AlgorithmParameters::OctetKeyPair(_) => Some(Self::Edwards),
            AlgorithmParameters::OctetKey(_) => None,
        }
    }

    fn verifies(self, algorithm: Algorithm) -> bool {
        use Algorithm::{ES256, ES384, EdDSA, PS256, PS384, PS512, RS256, RS384, RS512};
        match self {
            Self::Rsa => matches!(algorithm, RS256 | RS384 | RS512 | PS256 | PS384 | PS512),
            Self::EllipticCurve => matches!(algorithm, ES256 | ES384),
            Self::Edwards => algorithm == EdDSA,
        }
    }
}

pub(crate) struct Verifier {
    issuers: Vec<Issuer>,
    http: reqwest::Client,
    /// Each issuer's keys by ID. Two issuers may name keys alike.
    keys: RwLock<HashMap<Url, HashMap<String, Key>>>,
    reloaded: Mutex<Option<Instant>>,
}

impl Verifier {
    /// Trusts the tokens that `issuers` sign, with keys found through the
    /// discovery of `OpenID` Connect.
    ///
    /// # Errors
    ///
    /// Fails when the HTTP client can't be built.
    #[allow(
        clippy::disallowed_methods,
        reason = "vcs fetches key sets, and only jf must carry a command's trace"
    )]
    pub(crate) fn new(issuers: Vec<Issuer>) -> anyhow::Result<Self> {
        // reqwest needs one TLS provider per process. A second install, from
        // main or a test, fails and changes nothing.
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        Ok(Self {
            issuers,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?,
            keys: RwLock::default(),
            reloaded: Mutex::default(),
        })
    }

    /// Verifies a bearer token and returns its principal.
    ///
    /// # Errors
    ///
    /// Fails on a missing, malformed, expired, or foreign token, or one
    /// signed by a key that no trusted issuer publishes.
    pub(crate) async fn verify(&self, authorization: Option<&str>) -> Result<Principal, AuthError> {
        let result = self.principal(authorization).await;
        if let Err(AuthError::Invalid(reason)) = &result {
            tracing::info!(%reason, "token refused");
        }
        result
    }

    async fn principal(&self, authorization: Option<&str>) -> Result<Principal, AuthError> {
        // The scheme's name is case-insensitive, as RFC 9110 decides.
        let token = authorization
            .and_then(|value| value.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
            .map(|(_, token)| token.trim())
            .ok_or(AuthError::Missing)?;
        let header = jsonwebtoken::decode_header(token)
            .map_err(|err| AuthError::Invalid(err.to_string()))?;
        let kid = header
            .kid
            .ok_or_else(|| AuthError::Invalid("the token names no key".to_owned()))?;

        let has_key = |keys: &HashMap<Url, HashMap<String, Key>>| {
            keys.values().any(|issuer| issuer.contains_key(&kid))
        };
        if !has_key(&*self.keys.read().await) {
            self.reload().await;
        }

        // The first key named `kid` whose issuer signed the token wins.
        let keys = self.keys.read().await;
        let mut verified = None;
        let mut last = "no trusted issuer publishes its key".to_owned();
        for key in keys.values().filter_map(|issuer| issuer.get(&kid)) {
            let algorithm = key.algorithm.unwrap_or(header.alg);
            if !key.family.verifies(algorithm) {
                continue;
            }
            let mut validation = Validation::new(algorithm);
            validation.set_audience(&[AUDIENCE]);
            validation.set_issuer(&[&key.issuer]);
            validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
            match jsonwebtoken::decode::<Claims>(token, &key.decoding, &validation) {
                Ok(data) => {
                    verified = Some((data.claims, key.org.clone()));
                    break;
                }
                Err(err) => last = err.to_string(),
            }
        }
        let (claims, org) = verified.ok_or(AuthError::Invalid(last))?;

        let mut delegation = vec![];
        let mut actor = claims.act;
        while let Some(next) = actor {
            delegation.push(next.sub);
            actor = next.act.map(|boxed| *boxed);
        }
        Ok(Principal {
            subject: claims.sub,
            delegation,
            scopes: claims.scope.split_whitespace().map(str::to_owned).collect(),
            org,
        })
    }

    /// Loads every issuer's key set again, at most once per `RELOAD_EVERY`.
    /// An issuer that can't answer keeps its old keys, and vcs logs why.
    async fn reload(&self) {
        let mut reloaded = self.reloaded.lock().await;
        if reloaded.is_some_and(|at| at.elapsed() < RELOAD_EVERY) {
            return;
        }
        *reloaded = Some(Instant::now());
        tracing::info!(issuers = self.issuers.len(), "key sets reload");

        for Issuer { org, url: issuer } in &self.issuers {
            match self.key_set(issuer).await {
                Ok(set) => {
                    let mut keys = HashMap::new();
                    for jwk in set.keys {
                        let (Some(kid), Some(family)) =
                            (jwk.common.key_id.clone(), Family::of(&jwk.algorithm))
                        else {
                            continue;
                        };
                        let algorithm = jwk
                            .common
                            .key_algorithm
                            .and_then(|algorithm| algorithm.to_string().parse().ok());
                        if let Ok(decoding) = DecodingKey::from_jwk(&jwk) {
                            keys.insert(
                                kid,
                                Key {
                                    issuer: issuer.as_str().trim_end_matches('/').to_owned(),
                                    org: org.clone(),
                                    decoding,
                                    family,
                                    algorithm,
                                },
                            );
                        }
                    }
                    self.keys.write().await.insert(issuer.clone(), keys);
                }
                // The issuer keeps the keys it published last.
                Err(err) => tracing::warn!(%issuer, error = %err, "key set unavailable"),
            }
        }
    }

    async fn key_set(&self, issuer: &Url) -> anyhow::Result<JwkSet> {
        let discovery = format!(
            "{}/.well-known/openid-configuration",
            issuer.as_str().trim_end_matches('/')
        );
        let discovery: Discovery = self
            .http
            .get(discovery)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(self
            .http
            .get(discovery.jwks_uri)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

#[cfg(test)]
mod testing;

#[cfg(test)]
pub(crate) use testing::TestKey;
#[cfg(test)]
pub(crate) use testing::issuer_with;
#[cfg(test)]
pub(crate) use testing::token_with;

#[cfg(test)]
mod tests {
    use jsonwebtoken::EncodingKey;
    use jsonwebtoken::Header;

    use super::testing::TestKey;
    use super::testing::base64url;
    use super::testing::issuer_serving;
    use super::testing::issuer_with;
    use super::testing::token_with;
    use super::*;

    fn principal(scope: &str) -> Principal {
        Principal {
            subject: "ada".to_owned(),
            delegation: vec![],
            scopes: scope.split_whitespace().map(str::to_owned).collect(),
            org: "acme".to_owned(),
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

    #[tokio::test]
    async fn a_request_without_a_bearer_token_is_refused() {
        let verifier = Verifier::new(vec![]).unwrap();

        assert!(matches!(
            verifier.verify(None).await,
            Err(AuthError::Missing)
        ));
        assert!(matches!(
            verifier.verify(Some("Basic abc")).await,
            Err(AuthError::Missing)
        ));
    }

    #[tokio::test]
    async fn a_malformed_token_is_invalid() {
        let verifier = Verifier::new(vec![]).unwrap();

        assert!(matches!(
            verifier.verify(Some("Bearer not.a.token")).await,
            Err(AuthError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn an_issuer_grants_nothing_outside_its_organization() {
        let key = TestKey::new();
        let other = issuer_with("other", &key).await;
        let verifier = Verifier::new(vec![Issuer::new("other", other.clone())]).unwrap();
        let token = token_with(&other, &key, "sync:write:acme");

        let principal = verifier
            .verify(Some(&format!("Bearer {token}")))
            .await
            .unwrap();

        assert!(!principal.may(Access::Read, "acme"));
    }

    #[tokio::test]
    async fn two_issuers_may_name_their_keys_alike() {
        let (one, two) = (TestKey::new(), TestKey::new());
        let first = issuer_with("acme", &one).await;
        let second = issuer_with("other", &two).await;
        let verifier = Verifier::new(vec![
            Issuer::new("acme", first.clone()),
            Issuer::new("acme", second.clone()),
        ])
        .unwrap();

        for (issuer, key) in [(&first, &one), (&second, &two)] {
            let token = token_with(issuer, key, "sync:read:acme");
            assert!(
                verifier
                    .verify(Some(&format!("Bearer {token}")))
                    .await
                    .is_ok(),
                "{issuer}"
            );
        }
    }

    #[tokio::test]
    async fn a_key_set_without_algorithms_still_verifies() {
        let key = TestKey::new();
        let mut jwk = key.jwk();
        jwk.as_object_mut().unwrap().remove("alg");
        let issuer = issuer_serving("acme", jwk).await;
        let verifier = Verifier::new(vec![Issuer::new("acme", issuer.clone())]).unwrap();

        let token = token_with(&issuer, &key, "sync:read:acme");

        assert!(
            verifier
                .verify(Some(&format!("Bearer {token}")))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn a_symmetric_key_in_a_key_set_is_refused() {
        let secret = b"published, so anyone could sign with it";
        let issuer = issuer_serving(
            "acme",
            serde_json::json!({ "kty": "oct", "kid": "test", "alg": "HS256", "k": base64url(secret) }),
        )
        .await;
        let verifier = Verifier::new(vec![Issuer::new("acme", issuer.clone())]).unwrap();
        let mut header = Header::new(jsonwebtoken::Algorithm::HS256);
        header.kid = Some("test".to_owned());
        let claims = serde_json::json!({
            "iss": issuer.as_str(), "sub": "mallory", "aud": "jjforge",
            "exp": 4_102_444_800_u64, "scope": "sync:write:acme",
        });
        let token =
            jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap();

        assert!(
            verifier
                .verify(Some(&format!("Bearer {token}")))
                .await
                .is_err()
        );
    }

    // jsonwebtoken 9 skips a time claim of the wrong type instead of refusing
    // it (GHSA-h395-gr6q-cpjc), so a string `exp` would never expire.
    #[tokio::test]
    async fn a_time_claim_that_is_not_a_number_is_refused() {
        let key = TestKey::new();
        let issuer = issuer_with("acme", &key).await;
        let verifier = Verifier::new(vec![Issuer::new("acme", issuer.clone())]).unwrap();
        let mut header = Header::new(jsonwebtoken::Algorithm::EdDSA);
        header.kid = Some("test".to_owned());

        for (claim, value) in [("exp", "1"), ("nbf", "4102444800")] {
            let mut claims = serde_json::json!({
                "iss": issuer.as_str(), "sub": "ada", "aud": "jjforge",
                "exp": 4_102_444_800_u64, "scope": "sync:read:acme",
            });
            claims[claim] = value.into();
            let token =
                jsonwebtoken::encode(&header, &claims, &EncodingKey::from_ed_der(&key.pkcs8))
                    .unwrap();

            assert!(
                matches!(
                    verifier.verify(Some(&format!("Bearer {token}"))).await,
                    Err(AuthError::Invalid(_))
                ),
                "{claim}"
            );
        }
    }
}

//! Sessions and challenges that vcs seals with a key every replica shares,
//! so it keeps no state between requests.
//!
//! A challenge is the time it was issued, 16 random bytes, and a tag over
//! both. A session is its claims as JSON and a tag over them, each in hex,
//! joined by a period.

use std::str::FromStr;
use std::time::Duration;

use ring::hmac;
use ring::rand::SecureRandom as _;
use ring::rand::SystemRandom;
use serde::Deserialize;
use serde::Serialize;

use super::AuthError;

/// Separates a challenge's tag from a session's, so neither passes for the
/// other.
const CHALLENGE: &[u8] = b"jjforge challenge\0";
const SESSION: &[u8] = b"jjforge session\0";

const ISSUED: usize = 8;
const RANDOM: usize = 16;
const TAG: usize = 32;

/// What a session grants, from the certificate that opened it.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Claims {
    pub(crate) sub: String,
    pub(crate) delegation: Vec<String>,
    pub(crate) scopes: Vec<String>,
    pub(crate) orgs: Vec<String>,
    /// Seconds since the epoch.
    pub(crate) exp: u64,
}

/// The key that seals sessions and challenges.
#[derive(Clone)]
pub(crate) struct SessionKey(hmac::Key);

/// A key in hex, as `VCS_SESSION_KEY` holds it.
impl FromStr for SessionKey {
    type Err = String;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(key).map_err(|err| err.to_string())?;
        Self::new(&bytes).map_err(|err| err.to_string())
    }
}

impl SessionKey {
    /// A key of 32 bytes or more, such as one shared by every replica.
    ///
    /// # Errors
    ///
    /// Fails on a shorter key.
    pub(crate) fn new(bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(bytes.len() >= TAG, "a session key has at least {TAG} bytes");
        Ok(Self(hmac::Key::new(hmac::HMAC_SHA256, bytes)))
    }

    /// A key of this replica alone, whose sessions no other replica opens.
    ///
    /// # Errors
    ///
    /// Fails when the system has no randomness to give.
    pub(crate) fn random() -> anyhow::Result<Self> {
        let mut bytes = [0; TAG];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| anyhow::anyhow!("no randomness"))?;
        Self::new(&bytes)
    }

    fn tag(&self, domain: &[u8], data: &[u8]) -> hmac::Tag {
        let mut context = hmac::Context::with_key(&self.0);
        context.update(domain);
        context.update(data);
        context.sign()
    }

    fn verify(&self, domain: &[u8], data: &[u8], tag: &[u8]) -> bool {
        hmac::verify(&self.0, &[domain, data].concat(), tag).is_ok()
    }

    pub(super) fn challenge(&self, now: u64) -> anyhow::Result<Vec<u8>> {
        let mut nonce = now.to_be_bytes().to_vec();
        let mut random = [0; RANDOM];
        SystemRandom::new()
            .fill(&mut random)
            .map_err(|_| anyhow::anyhow!("no randomness"))?;
        nonce.extend_from_slice(&random);
        let tag = self.tag(CHALLENGE, &nonce);
        nonce.extend_from_slice(tag.as_ref());
        Ok(nonce)
    }

    pub(super) fn check_challenge(&self, nonce: &[u8], now: u64, lifetime: Duration) -> bool {
        if nonce.len() != ISSUED + RANDOM + TAG {
            return false;
        }
        let (data, tag) = nonce.split_at(ISSUED + RANDOM);
        let mut issued = [0; ISSUED];
        issued.copy_from_slice(&data[..ISSUED]);
        let issued = u64::from_be_bytes(issued);
        self.verify(CHALLENGE, data, tag) && issued <= now && now - issued <= lifetime.as_secs()
    }

    /// Seals `claims` as a session.
    pub(crate) fn seal(&self, claims: &Claims) -> String {
        // Claims of strings and numbers always encode.
        let json = serde_json::to_vec(claims).unwrap_or_default();
        let tag = self.tag(SESSION, &json);
        format!("{}.{}", hex::encode(&json), hex::encode(tag.as_ref()))
    }

    /// Opens a session that this key sealed and that hasn't expired.
    pub(super) fn open(&self, token: &str, now: u64) -> Result<Claims, AuthError> {
        let invalid = |reason: &str| AuthError::Invalid(reason.to_owned());
        let (json, tag) = token
            .split_once('.')
            .ok_or_else(|| invalid("the session is malformed"))?;
        let (json, tag) = hex::decode(json)
            .and_then(|json| Ok((json, hex::decode(tag)?)))
            .map_err(|_| invalid("the session is malformed"))?;
        if !self.verify(SESSION, &json, &tag) {
            return Err(invalid("another key sealed the session"));
        }
        let claims: Claims =
            serde_json::from_slice(&json).map_err(|err| AuthError::Invalid(err.to_string()))?;
        if claims.exp <= now {
            return Err(invalid("the session expired"));
        }
        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIFETIME: Duration = Duration::from_secs(60);

    fn claims(exp: u64) -> Claims {
        Claims {
            sub: "ada".to_owned(),
            delegation: vec![],
            scopes: vec!["sync:read:acme".to_owned()],
            orgs: vec!["acme".to_owned()],
            exp,
        }
    }

    #[test]
    fn a_sealed_session_opens_until_it_expires() {
        let key = SessionKey::random().unwrap();
        let token = key.seal(&claims(100));

        assert_eq!(key.open(&token, 99).unwrap().sub, "ada");
        assert!(key.open(&token, 100).is_err());
    }

    #[test]
    fn another_key_opens_no_session() {
        let token = SessionKey::random().unwrap().seal(&claims(100));

        assert!(SessionKey::random().unwrap().open(&token, 1).is_err());
    }

    #[test]
    fn a_changed_session_opens_no_more() {
        let key = SessionKey::random().unwrap();
        let token = key.seal(&claims(100));
        let other = key.seal(&claims(200));
        let (json, _) = other.split_once('.').unwrap();
        let (_, tag) = token.split_once('.').unwrap();

        assert!(key.open(&format!("{json}.{tag}"), 1).is_err());
    }

    #[test]
    fn a_challenge_holds_for_its_lifetime_only() {
        let key = SessionKey::random().unwrap();
        let nonce = key.challenge(1_000).unwrap();

        assert!(key.check_challenge(&nonce, 1_060, LIFETIME));
        assert!(!key.check_challenge(&nonce, 1_061, LIFETIME));
        assert!(!key.check_challenge(&nonce, 999, LIFETIME));
    }

    #[test]
    fn a_session_is_no_challenge() {
        let key = SessionKey::random().unwrap();
        let mut nonce = 1_000_u64.to_be_bytes().to_vec();
        nonce.extend_from_slice(&[0; RANDOM]);
        nonce.extend_from_slice(key.tag(SESSION, &nonce).as_ref());

        assert!(!key.check_challenge(&nonce, 1_000, LIFETIME));
    }

    #[test]
    fn a_short_key_is_refused() {
        assert!(SessionKey::new(&[0; 16]).is_err());
    }
}

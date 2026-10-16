//! Keys, issuers, and tokens for the tests of vcs. Each test generates its
//! key, so no private key lives in the repository.

#![allow(clippy::unwrap_used, reason = "a panic in a test is the failure")]

use axum::Router;
use axum::routing::get;
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use url::Url;

/// An Ed25519 key that each test generates, so no private key lives in
/// the repository.
pub(crate) struct TestKey {
    pub(crate) pkcs8: Vec<u8>,
    public: Vec<u8>,
}

impl TestKey {
    pub(crate) fn new() -> Self {
        use ring::signature::KeyPair as _;
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)
            .unwrap()
            .as_ref()
            .to_vec();
        let public = ring::signature::Ed25519KeyPair::from_pkcs8(&pkcs8)
            .unwrap()
            .public_key()
            .as_ref()
            .to_vec();
        Self { pkcs8, public }
    }

    pub(crate) fn jwk(&self) -> serde_json::Value {
        serde_json::json!({
            "kty": "OKP",
            "crv": "Ed25519",
            "kid": "test",
            "alg": "EdDSA",
            "x": base64url(&self.public),
        })
    }
}

/// Unpadded base64url, as a JWK writes a key.
pub(crate) fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, byte)| n | u32::from(*byte) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
        }
    }
    out
}

/// An issuer at `/{path}` that publishes `keys`.
pub(crate) async fn issuer_serving(path: &str, keys: serde_json::Value) -> Url {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/{path}", listener.local_addr().unwrap());
    let discovery = serde_json::json!({ "jwks_uri": format!("{base}/jwks") }).to_string();
    let keys = serde_json::json!({ "keys": [keys] }).to_string();
    let app = Router::new()
        .route(
            &format!("/{path}/.well-known/openid-configuration"),
            get(move || async move { discovery }),
        )
        .route(&format!("/{path}/jwks"), get(move || async move { keys }));
    tokio::spawn(async move { axum::serve(listener, app).await });
    Url::parse(&base).unwrap()
}

pub(crate) async fn issuer_with(path: &str, key: &TestKey) -> Url {
    issuer_serving(path, key.jwk()).await
}

pub(crate) fn token_with(issuer: &Url, key: &TestKey, scope: &str) -> String {
    let mut header = Header::new(jsonwebtoken::Algorithm::EdDSA);
    header.kid = Some("test".to_owned());
    let claims = serde_json::json!({
        "iss": issuer.as_str(),
        "sub": "ada",
        "aud": "jjforge",
        "exp": 4_102_444_800_u64,
        "scope": scope,
    });
    jsonwebtoken::encode(&header, &claims, &EncodingKey::from_ed_der(&key.pkcs8)).unwrap()
}

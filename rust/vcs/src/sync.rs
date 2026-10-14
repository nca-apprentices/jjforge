//! `sync/v1`, the routes `jf` pushes, fetches, and clones through, as
//! shared/proto/sync/v1/sync.proto describes them. Each route verifies the
//! token and calls the store crate's `Repo`.

use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::get;
use axum::routing::post;
use jjforge_proto::sync::v1::HashList;
use jjforge_proto::sync::v1::Head;
use jjforge_proto::sync::v1::ObjectBundle;
use jjforge_proto::sync::v1::PublishRequest;
use jjforge_proto::sync::v1::PublishResponse;
use jjforge_proto::sync::v1::Uploaded;
use jjforge_store::Author;
use jjforge_store::Published;
use jjforge_store::Repo;
use jjforge_store::RepoError;
use jjforge_store::Repos;
use prost::Message;
use serde::Deserialize;

use crate::auth::Access;
use crate::auth::AuthError;
use crate::auth::Verifier;

/// The largest request body, so a hostile client can't exhaust memory. An
/// upload above it answers 413, and `jf` stays below it.
const BODY_LIMIT: usize = 64 << 20;

const PROTOBUF: &str = "application/x-protobuf";

/// The most IDs one request names, as many as `jf` sends, so a request
/// can't ask for one large object a million times.
const MAX_IDS: usize = 1000;

#[derive(Clone)]
struct Sync {
    repos: Arc<Repos>,
    verifier: Arc<Verifier>,
}

pub(crate) fn router(repos: Arc<Repos>, verifier: Arc<Verifier>) -> Router {
    Router::new()
        .route("/sync/v1/{org}/{repo}/head", get(head))
        .route("/sync/v1/{org}/{repo}/operations", get(operations))
        .route("/sync/v1/{org}/{repo}/objects/missing", post(missing))
        .route("/sync/v1/{org}/{repo}/objects/fetch", post(fetch))
        .route("/sync/v1/{org}/{repo}/objects/{id}", get(object))
        .route("/sync/v1/{org}/{repo}/objects", post(upload))
        .route("/sync/v1/{org}/{repo}/publish", post(publish))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(axum::middleware::map_response(as_problem))
        .with_state(Sync { repos, verifier })
}

/// An `application/problem+json` answer with the problem codes of the REST
/// API, so `jf` maps both to the same exit codes.
struct Problem {
    status: StatusCode,
    /// `None` for a failure of vcs itself, which the contract gives no code.
    code: Option<&'static str>,
    detail: String,
    head: Option<Vec<u8>>,
}

impl Problem {
    fn new(status: StatusCode, code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            status,
            code: Some(code),
            detail: detail.into(),
            head: None,
        }
    }

    fn invalid(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid", detail)
    }

    fn not_found(org: &str, repo: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("repository {org}/{repo} doesn't exist"),
        )
    }
}

impl From<RepoError> for Problem {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::NotFound(_) => {
                Self::new(StatusCode::NOT_FOUND, "not_found", err.to_string())
            }
            RepoError::Invalid(_) => Self::invalid(err.to_string()),
            RepoError::Internal(_) => {
                tracing::error!(error = %err, "store failed");
                Self {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    code: None,
                    detail: "the store failed".to_owned(),
                    head: None,
                }
            }
        }
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "code": self.code,
            "title": self.status.canonical_reason().unwrap_or_default(),
            "status": self.status.as_u16(),
            "detail": self.detail,
        });
        let mut response = (
            self.status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            body.to_string(),
        )
            .into_response();
        if let Some(head) = self.head
            && let Ok(tag) = HeaderValue::try_from(etag(&head))
        {
            response.headers_mut().insert(header::ETAG, tag);
        }
        response
    }
}

/// Answers a failure that axum answers itself, such as a body above the
/// limit, as a problem too, as ADR 0001 decides for every error.
async fn as_problem(response: Response) -> Response {
    let status = response.status();
    let is_problem = response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|value| value == "application/problem+json");
    if !(status.is_client_error() || status.is_server_error()) || is_problem {
        return response;
    }
    let body = axum::body::to_bytes(response.into_body(), BODY_LIMIT)
        .await
        .unwrap_or_default();
    let detail = String::from_utf8_lossy(&body).into_owned();
    if status.is_server_error() {
        return Problem {
            status,
            code: None,
            detail,
            head: None,
        }
        .into_response();
    }
    Problem::new(status, "invalid", detail).into_response()
}

fn etag(operation_id: &[u8]) -> String {
    format!("\"{}\"", hex::encode(operation_id))
}

fn protobuf(message: &impl Message) -> Response {
    ([(header::CONTENT_TYPE, PROTOBUF)], message.encode_to_vec()).into_response()
}

fn decode<M: Message + Default>(body: &Bytes) -> Result<M, Problem> {
    M::decode(body.as_ref())
        .map_err(|err| Problem::invalid(format!("the body is malformed: {err}")))
}

fn ids(body: &Bytes) -> Result<Vec<Vec<u8>>, Problem> {
    let ids = decode::<HashList>(body)?.ids;
    if ids.len() > MAX_IDS {
        return Err(Problem::invalid(format!(
            "a request names at most {MAX_IDS} IDs"
        )));
    }
    Ok(ids)
}

impl Sync {
    /// Verifies the request's token and opens the repository. A principal
    /// without any access to the organization learns nothing, not even that
    /// it exists, as #27 decides.
    async fn open(
        &self,
        headers: &HeaderMap,
        org: &str,
        repo: &str,
        access: Access,
    ) -> Result<(Repo, Author), Problem> {
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        let principal = self
            .verifier
            .verify(authorization)
            .await
            .map_err(|err| match err {
                AuthError::Missing | AuthError::Invalid(_) => {
                    Problem::new(StatusCode::UNAUTHORIZED, "unauthenticated", err.to_string())
                }
            })?;
        if !principal.may(Access::Read, org) {
            return Err(Problem::not_found(org, repo));
        }
        if !principal.may(access, org) {
            return Err(Problem::new(
                StatusCode::FORBIDDEN,
                "forbidden",
                "the token doesn't allow writes to this repository",
            ));
        }
        // The span names whom the request was for, as ADR 0005 decides.
        let span = tracing::Span::current();
        span.record("jjforge.org.id", org);
        span.record("jjforge.repo.id", repo);
        span.record("jjforge.principal.id", principal.subject.as_str());
        let repo = self.repos.get(org, repo)?;
        Ok((
            repo,
            Author {
                subject: principal.subject,
                delegation: principal.delegation,
            },
        ))
    }
}

async fn head(
    State(sync): State<Sync>,
    Path((org, repo)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let (repo, _) = sync.open(&headers, &org, &repo, Access::Read).await?;
    let head = repo.head().await?;
    let mut response = protobuf(&Head {
        operation_id: head.operation_id.clone(),
    });
    if let Ok(tag) = HeaderValue::try_from(etag(&head.operation_id)) {
        response.headers_mut().insert(header::ETAG, tag);
    }
    Ok(response)
}

#[derive(Deserialize)]
struct Since {
    since: Option<String>,
}

async fn operations(
    State(sync): State<Sync>,
    Path((org, repo)): Path<(String, String)>,
    Query(query): Query<Since>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let (repo, _) = sync.open(&headers, &org, &repo, Access::Read).await?;
    let since = query
        .since
        .map(hex::decode)
        .transpose()
        .map_err(|_| Problem::invalid("since is not hexadecimal"))?;
    let objects = repo.operations_since(since.as_deref()).await?;
    Ok(protobuf(&ObjectBundle { objects }))
}

async fn missing(
    State(sync): State<Sync>,
    Path((org, repo)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Problem> {
    let (repo, _) = sync.open(&headers, &org, &repo, Access::Read).await?;
    let ids = ids(&body)?;
    Ok(protobuf(&HashList {
        ids: repo.missing(ids).await?,
    }))
}

async fn fetch(
    State(sync): State<Sync>,
    Path((org, repo)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Problem> {
    let (repo, _) = sync.open(&headers, &org, &repo, Access::Read).await?;
    let ids = ids(&body)?;
    Ok(protobuf(&ObjectBundle {
        objects: repo.fetch(ids).await?,
    }))
}

/// One object by ID. It never changes, so any cache may keep it.
async fn object(
    State(sync): State<Sync>,
    Path((org, repo, id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let (repo, _) = sync.open(&headers, &org, &repo, Access::Read).await?;
    let id = hex::decode(&id).map_err(|_| Problem::invalid("the ID is not hexadecimal"))?;
    let bytes = repo.fetch(vec![id]).await?.pop().unwrap_or_default();
    Ok((
        [
            (header::CONTENT_TYPE, PROTOBUF),
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn upload(
    State(sync): State<Sync>,
    Path((org, repo)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Problem> {
    let (repo, _) = sync.open(&headers, &org, &repo, Access::Write).await?;
    let objects = decode::<ObjectBundle>(&body)?.objects;
    Ok(protobuf(&Uploaded {
        segment_id: repo.upload(objects).await?,
    }))
}

/// Publishes when the head is still the one the client names, in the body
/// and in `If-Match`. Otherwise it answers 412 with the current head, as
/// #89 decides.
async fn publish(
    State(sync): State<Sync>,
    Path((org, repo)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Problem> {
    let (repo, author) = sync.open(&headers, &org, &repo, Access::Write).await?;
    let request = decode::<PublishRequest>(&body)?;
    if let Some(expected) = headers.get(header::IF_MATCH)
        && expected.as_bytes() != etag(&request.expected_operation_id).as_bytes()
    {
        return Err(Problem::invalid(
            "If-Match and the expected operation differ",
        ));
    }

    // jj-lib's futures aren't Send, so the publish runs on a thread of its
    // own.
    let runtime = tokio::runtime::Handle::current();
    let span = tracing::Span::current();
    let published = tokio::task::spawn_blocking(move || {
        span.in_scope(|| runtime.block_on(repo.publish(&request, &author)))
    })
    .await
    .map_err(|err| RepoError::Internal(err.into()))??;
    match published {
        Published::Moved(operation_id) => Ok(protobuf(&PublishResponse { operation_id })),
        Published::Stale(head) => {
            let mut problem = Problem::new(
                StatusCode::PRECONDITION_FAILED,
                "precondition_failed",
                "the head or a bookmark moved since the client fetched it",
            );
            problem.head = Some(head.operation_id);
            Err(problem)
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use jsonwebtoken::EncodingKey;
    use jsonwebtoken::Header;
    use tower::ServiceExt as _;
    use url::Url;

    use super::*;
    use crate::auth::Issuer;

    /// An Ed25519 key that each test generates, so no private key lives in
    /// the repository.
    struct TestKey {
        pkcs8: Vec<u8>,
        public: Vec<u8>,
    }

    impl TestKey {
        fn new() -> Self {
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

        fn jwk(&self) -> serde_json::Value {
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
    fn base64url(bytes: &[u8]) -> String {
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
    async fn issuer_serving(path: &str, keys: serde_json::Value) -> Url {
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

    async fn issuer_with(path: &str, key: &TestKey) -> Url {
        issuer_serving(path, key.jwk()).await
    }

    fn token_with(issuer: &Url, key: &TestKey, scope: &str) -> String {
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

    fn token(fixture: &Fixture, scope: &str) -> String {
        token_with(&fixture.issuer, &fixture.key, scope)
    }

    struct Fixture {
        router: Router,
        issuer: Url,
        key: TestKey,
        _cache: tempfile::TempDir,
    }

    async fn fixture() -> Fixture {
        let key = TestKey::new();
        let issuer = issuer_with("acme", &key).await;
        let cache = tempfile::tempdir().unwrap();
        let repos = Repos::open(&Url::parse("memory:///").unwrap(), cache.path()).unwrap();
        repos.get("acme", "demo").unwrap().create().await.unwrap();
        let verifier = Verifier::new(vec![Issuer::new("acme", issuer.clone())]).unwrap();
        Fixture {
            router: router(Arc::new(repos), Arc::new(verifier)),
            issuer,
            key,
            _cache: cache,
        }
    }

    async fn call(fixture: &Fixture, request: Request<Body>) -> Response {
        fixture.router.clone().oneshot(request).await.unwrap()
    }

    async fn code(response: Response) -> serde_json::Value {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["code"].clone()
    }

    async fn detail(response: Response) -> serde_json::Value {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["detail"].clone()
    }

    fn get_head(fixture: &Fixture, scope: Option<&str>) -> Request<Body> {
        let mut request = Request::get("/sync/v1/acme/demo/head");
        if let Some(scope) = scope {
            request = request.header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(fixture, scope)),
            );
        }
        request.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn head_answers_the_root_operation_of_a_new_repository() {
        let fixture = fixture().await;

        let response = call(&fixture, get_head(&fixture, Some("sync:read:acme"))).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::ETAG], etag(&[0; 32]).as_str());
    }

    #[tokio::test]
    async fn a_request_without_a_token_is_unauthenticated() {
        let fixture = fixture().await;

        let response = call(&fixture, get_head(&fixture, None)).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(code(response).await, "unauthenticated");
    }

    #[tokio::test]
    async fn a_principal_outside_the_organization_finds_nothing() {
        let fixture = fixture().await;

        let response = call(&fixture, get_head(&fixture, Some("sync:write:other"))).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(code(response).await, "not_found");
    }

    #[tokio::test]
    async fn a_reader_can_not_upload() {
        let fixture = fixture().await;
        let request = Request::post("/sync/v1/acme/demo/objects")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(&fixture, "sync:read:acme")),
            )
            .body(Body::from(ObjectBundle::default().encode_to_vec()))
            .unwrap();

        let response = call(&fixture, request).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(code(response).await, "forbidden");
    }

    #[tokio::test]
    async fn a_malformed_object_is_invalid() {
        let fixture = fixture().await;
        let bundle = ObjectBundle {
            objects: vec![b"not an object".to_vec()],
        };
        let request = Request::post("/sync/v1/acme/demo/objects")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(&fixture, "sync:write:acme")),
            )
            .body(Body::from(bundle.encode_to_vec()))
            .unwrap();

        let response = call(&fixture, request).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(code(response).await, "invalid");
    }

    #[tokio::test]
    async fn a_publish_from_a_stale_head_names_the_current_one() {
        let fixture = fixture().await;
        let stale = PublishRequest {
            expected_operation_id: vec![1; 32],
            ..PublishRequest::default()
        };
        let request = Request::post("/sync/v1/acme/demo/publish")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(&fixture, "sync:write:acme")),
            )
            .header(header::IF_MATCH, etag(&[1; 32]))
            .body(Body::from(stale.encode_to_vec()))
            .unwrap();

        let response = call(&fixture, request).await;

        assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
        assert_eq!(response.headers()[header::ETAG], etag(&[0; 32]).as_str());
        assert_eq!(code(response).await, "precondition_failed");
    }

    #[tokio::test]
    async fn a_request_for_too_many_objects_is_invalid() {
        let fixture = fixture().await;
        let ids = HashList {
            ids: vec![vec![1; 32]; 1001],
        };
        let request = Request::post("/sync/v1/acme/demo/objects/fetch")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(&fixture, "sync:read:acme")),
            )
            .body(Body::from(ids.encode_to_vec()))
            .unwrap();

        let response = call(&fixture, request).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(code(response).await, "invalid");
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

    #[tokio::test]
    async fn the_bearer_scheme_is_case_insensitive() {
        let fixture = fixture().await;
        let request = Request::get("/sync/v1/acme/demo/head")
            .header(
                header::AUTHORIZATION,
                format!("bearer {}", token(&fixture, "sync:read:acme")),
            )
            .body(Body::empty())
            .unwrap();

        let response = call(&fixture, request).await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn a_body_above_the_limit_answers_a_problem() {
        let fixture = fixture().await;
        let request = Request::post("/sync/v1/acme/demo/objects")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(&fixture, "sync:write:acme")),
            )
            .body(Body::from(vec![0; BODY_LIMIT + 1]))
            .unwrap();

        let response = call(&fixture, request).await;

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "application/problem+json"
        );
    }

    #[tokio::test]
    async fn a_repository_that_was_never_created_is_not_found() {
        let fixture = fixture().await;
        let request = Request::get("/sync/v1/acme/nothing/head")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(&fixture, "sync:read:acme")),
            )
            .body(Body::empty())
            .unwrap();

        let response = call(&fixture, request).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            detail(response).await,
            "repository acme/nothing doesn't exist"
        );
    }

    #[tokio::test]
    async fn a_refused_token_gets_no_parser_detail() {
        let fixture = fixture().await;
        let request = Request::get("/sync/v1/acme/demo/head")
            .header(header::AUTHORIZATION, "Bearer abc.def.ghi")
            .body(Body::empty())
            .unwrap();

        let response = call(&fixture, request).await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(detail(response).await, "the token is invalid");
    }
}

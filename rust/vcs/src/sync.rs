//! `sync/v1`, the routes `jf` pushes, fetches, and clones through, as
//! shared/proto/sync/v1/sync.proto describes them. A handler takes a
//! [`Reader`] or a [`Writer`], so no route reaches a repository before the
//! [`Gate`] lets it.

mod problem;

use std::net::SocketAddr;

use axum::Router;
use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::extract::FromRequestParts;
use axum::extract::Path;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::http::header;
use axum::http::request::Parts;
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
use jjforge_store::ObjectId;
use jjforge_store::OperationId;
use jjforge_store::Published;
use jjforge_store::server::Author;
use jjforge_store::server::Repo;
use prost::Message;
use serde::Deserialize;

use self::problem::Problem;
use self::problem::as_problem;
use self::problem::etag;
use self::problem::set_etag;
use crate::auth::Access;
use crate::gate::Gate;
use crate::telemetry;

/// The largest request body, so a hostile client can't exhaust memory. An
/// upload above it answers 413, and `jf` stays below it.
const BODY_LIMIT: usize = 64 << 20;

const PROTOBUF: &str = "application/x-protobuf";

/// The most IDs one request names, as many as `jf` sends, so a request
/// can't ask for one large object a million times.
const MAX_IDS: usize = 1000;

/// Serves `sync/v1` on `addr` in a task of its own.
pub(crate) async fn serve(addr: SocketAddr, gate: Gate) -> anyhow::Result<()> {
    // Meter labels gRPC methods, and a sync/v1 path holds IDs, which never
    // label a metric, as ADR 0005 decides. The span of each request records
    // the route instead.
    let router = router(gate).layer(axum::middleware::from_fn(telemetry::trace_http));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(sync_addr = %addr, "sync/v1 listening");
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            tracing::error!(error = %err, "sync/v1 stopped");
        }
    });
    Ok(())
}

fn router(gate: Gate) -> Router {
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
        .with_state(gate)
}

/// The repository a route names, which the request's token may read.
struct Reader(Repo);

/// The repository a route names, which the request's token may write, and
/// the author of what it writes.
struct Writer(Repo, Author);

impl FromRequestParts<Gate> for Reader {
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, gate: &Gate) -> Result<Self, Problem> {
        let (repo, _) = open(parts, gate, Access::Read).await?;
        Ok(Self(repo))
    }
}

impl FromRequestParts<Gate> for Writer {
    type Rejection = Problem;

    async fn from_request_parts(parts: &mut Parts, gate: &Gate) -> Result<Self, Problem> {
        let (repo, author) = open(parts, gate, Access::Write).await?;
        Ok(Self(repo, author))
    }
}

/// The IDs in a route's path. A route may name more, such as an object's.
#[derive(Deserialize)]
struct RepoPath {
    org: String,
    repo: String,
}

async fn open(parts: &mut Parts, gate: &Gate, access: Access) -> Result<(Repo, Author), Problem> {
    let Path(path) = Path::<RepoPath>::from_request_parts(parts, gate)
        .await
        .map_err(|err| Problem::invalid(err.body_text()))?;
    let authorization = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    Ok(gate
        .open(authorization, &path.org, &path.repo, access)
        .await?)
}

fn protobuf(message: &impl Message) -> Response {
    ([(header::CONTENT_TYPE, PROTOBUF)], message.encode_to_vec()).into_response()
}

fn decode<M: Message + Default>(body: &Bytes) -> Result<M, Problem> {
    M::decode(body.as_ref())
        .map_err(|err| Problem::invalid(format!("the body is malformed: {err}")))
}

fn ids(body: &Bytes) -> Result<Vec<ObjectId>, Problem> {
    let ids = decode::<HashList>(body)?.ids;
    if ids.len() > MAX_IDS {
        return Err(Problem::invalid(format!(
            "a request names at most {MAX_IDS} IDs"
        )));
    }
    ids.iter()
        .map(|id| {
            ObjectId::try_from(id.as_slice()).map_err(|err| Problem::invalid(err.to_string()))
        })
        .collect()
}

async fn head(Reader(repo): Reader) -> Result<Response, Problem> {
    let head = repo.head().await?;
    let mut response = protobuf(&Head {
        operation_id: head.to_vec(),
    });
    set_etag(&mut response, head);
    Ok(response)
}

#[derive(Deserialize)]
struct Since {
    since: Option<String>,
}

async fn operations(Reader(repo): Reader, Query(query): Query<Since>) -> Result<Response, Problem> {
    let since = query
        .since
        .map(|since| since.parse::<OperationId>())
        .transpose()
        .map_err(|err| Problem::invalid(format!("since: {err}")))?;
    let objects = repo.operations_since(since).await?;
    Ok(protobuf(&ObjectBundle { objects }))
}

async fn missing(Reader(repo): Reader, body: Bytes) -> Result<Response, Problem> {
    let missing = repo.missing(&ids(&body)?).await?;
    Ok(protobuf(&HashList {
        ids: missing.iter().map(ObjectId::to_vec).collect(),
    }))
}

async fn fetch(Reader(repo): Reader, body: Bytes) -> Result<Response, Problem> {
    Ok(protobuf(&ObjectBundle {
        objects: repo.fetch(&ids(&body)?).await?,
    }))
}

#[derive(Deserialize)]
struct ObjectPath {
    id: String,
}

/// One object by ID. It never changes, so any cache may keep it.
async fn object(Reader(repo): Reader, Path(path): Path<ObjectPath>) -> Result<Response, Problem> {
    let id = path
        .id
        .parse::<ObjectId>()
        .map_err(|err| Problem::invalid(err.to_string()))?;
    let bytes = repo.fetch(&[id]).await?.pop().unwrap_or_default();
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

async fn upload(Writer(repo, _): Writer, body: Bytes) -> Result<Response, Problem> {
    let objects = decode::<ObjectBundle>(&body)?.objects;
    Ok(protobuf(&Uploaded {
        segment_id: repo.upload(objects).await?.to_vec(),
    }))
}

/// Publishes when the head is still the one the client names, in the body
/// and in `If-Match`. Otherwise it answers 412 with the current head, as
/// #89 decides.
async fn publish(
    Writer(repo, author): Writer,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Problem> {
    let request = decode::<PublishRequest>(&body)?;
    if let Some(expected) = headers.get(header::IF_MATCH)
        && expected.as_bytes() != etag(&request.expected_operation_id).as_bytes()
    {
        return Err(Problem::invalid(
            "If-Match and the expected operation differ",
        ));
    }

    match repo.publish(request, author).await? {
        Published::Moved(operation_id) => Ok(protobuf(&PublishResponse {
            operation_id: operation_id.to_vec(),
        })),
        Published::Stale(head) => Err(Problem::stale(head)),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::Request;
    use axum::http::StatusCode;
    use tower::ServiceExt as _;
    use url::Url;

    use jjforge_store::server::Repos;

    use super::*;
    use crate::auth::Issuer;
    use crate::auth::TestKey;
    use crate::auth::Verifier;
    use crate::auth::issuer_with;
    use crate::auth::token_with;

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
            router: router(Gate::new(repos, verifier)),
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

    #[tokio::test]
    async fn a_refused_request_names_who_asked() {
        let fixture = fixture().await;
        let app = fixture
            .router
            .clone()
            .layer(axum::middleware::from_fn(crate::telemetry::trace_http));
        let request = Request::post("/sync/v1/acme/demo/objects")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(&fixture, "sync:read:acme")),
            )
            .body(Body::empty())
            .unwrap();

        let (response, spans) = crate::telemetry::finished_spans(app.oneshot(request)).await;

        assert_eq!(response.unwrap().status(), StatusCode::FORBIDDEN);
        assert_eq!(
            crate::telemetry::attribute(&spans, "jjforge.principal.id").as_deref(),
            Some("ada")
        );
        assert_eq!(
            crate::telemetry::attribute(&spans, "jjforge.refusal").as_deref(),
            Some("forbidden")
        );
    }

    #[tokio::test]
    async fn a_publish_traces_what_it_did_down_to_the_object_store() {
        let fixture = fixture().await;
        let app = fixture
            .router
            .clone()
            .layer(axum::middleware::from_fn(crate::telemetry::trace_http));
        let root = OperationId::root();
        let unchanged = PublishRequest {
            expected_operation_id: root.to_vec(),
            ..PublishRequest::default()
        };
        let request = Request::post("/sync/v1/acme/demo/publish")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", token(&fixture, "sync:write:acme")),
            )
            .body(Body::from(unchanged.encode_to_vec()))
            .unwrap();

        let (response, spans) = crate::telemetry::finished_spans(app.oneshot(request)).await;

        assert_eq!(response.unwrap().status(), StatusCode::OK);
        let attribute = |key| crate::telemetry::attribute(&spans, key);
        assert_eq!(
            attribute("jjforge.publish.outcome").as_deref(),
            Some("unchanged")
        );
        assert_eq!(attribute("jjforge.operation.id"), Some(root.to_string()));
        assert_eq!(
            attribute("jjforge.object_store.operation").as_deref(),
            Some("get")
        );
    }

    #[tokio::test]
    async fn every_route_is_traceable_with_its_org_repo_and_principal() {
        const TRACE: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
        let fixture = fixture().await;
        let app = fixture
            .router
            .clone()
            .layer(axum::middleware::from_fn(crate::telemetry::trace_http));
        let id = "07".repeat(32);
        let routes = [
            ("GET", "head".to_owned()),
            ("GET", "operations".to_owned()),
            ("POST", "objects/missing".to_owned()),
            ("POST", "objects/fetch".to_owned()),
            ("GET", format!("objects/{id}")),
            ("POST", "objects".to_owned()),
            ("POST", "publish".to_owned()),
        ];

        for (method, route) in routes {
            let request = Request::builder()
                .method(method)
                .uri(format!("/sync/v1/acme/demo/{route}"))
                .header("traceparent", format!("00-{TRACE}-00f067aa0ba902b7-01"))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", token(&fixture, "sync:write:acme")),
                )
                .body(Body::empty())
                .unwrap();

            let (_, spans) = crate::telemetry::finished_spans(app.clone().oneshot(request)).await;

            assert!(
                spans
                    .iter()
                    .all(|span| span.span_context.trace_id().to_string() == TRACE),
                "{route}"
            );
            for (key, value) in [
                ("jjforge.org.id", "acme"),
                ("jjforge.repo.id", "demo"),
                ("jjforge.principal.id", "ada"),
            ] {
                assert_eq!(
                    crate::telemetry::attribute(&spans, key).as_deref(),
                    Some(value),
                    "{route}"
                );
            }
        }
    }

    #[tokio::test]
    async fn a_request_that_reloads_the_key_sets_shows_it() {
        let fixture = fixture().await;
        let app = fixture
            .router
            .clone()
            .layer(axum::middleware::from_fn(crate::telemetry::trace_http));

        let (_, spans) = crate::telemetry::finished_spans(
            app.oneshot(get_head(&fixture, Some("sync:read:acme"))),
        )
        .await;

        assert!(crate::telemetry::events(&spans).contains(&"key sets reload".to_owned()));
    }

    #[tokio::test]
    async fn a_request_without_a_token_names_why_it_was_refused() {
        let fixture = fixture().await;
        let app = fixture
            .router
            .clone()
            .layer(axum::middleware::from_fn(crate::telemetry::trace_http));

        let (_, spans) =
            crate::telemetry::finished_spans(app.oneshot(get_head(&fixture, None))).await;

        assert_eq!(
            crate::telemetry::attribute(&spans, "jjforge.refusal").as_deref(),
            Some("unauthenticated")
        );
    }
}

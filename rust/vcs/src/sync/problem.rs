//! How `sync/v1` answers a failure: an `application/problem+json` body with
//! the problem codes of the REST API, so `jf` maps both to the same exit
//! codes, as ADR 0001 decides for every error.

use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::response::Response;
use jjforge_store::OperationId;
use jjforge_store::server::RepoError;

use super::BODY_LIMIT;
use crate::gate::Refused;

const PROBLEM_JSON: &str = "application/problem+json";

pub(crate) struct Problem {
    status: StatusCode,
    /// `None` for a failure of vcs itself, which the contract gives no code.
    code: Option<&'static str>,
    detail: String,
    /// The current head, which a stale publish names in its `ETag`.
    head: Option<OperationId>,
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

    fn internal(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            code: None,
            detail: detail.into(),
            head: None,
        }
    }

    pub(crate) fn invalid(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid", detail)
    }

    /// A publish whose head or bookmark moved, answered with 412 and the
    /// current head, as #89 decides.
    pub(crate) fn stale(head: OperationId) -> Self {
        Self {
            head: Some(head),
            ..Self::new(
                StatusCode::PRECONDITION_FAILED,
                "precondition_failed",
                "the head or a bookmark moved since the client fetched it",
            )
        }
    }
}

impl From<Refused> for Problem {
    fn from(err: Refused) -> Self {
        match err {
            Refused::Unauthenticated(_) => {
                Self::new(StatusCode::UNAUTHORIZED, "unauthenticated", err.to_string())
            }
            Refused::Forbidden => Self::new(
                StatusCode::FORBIDDEN,
                "forbidden",
                "the token doesn't allow writes to this repository",
            ),
            Refused::Repo(err) => err.into(),
        }
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
                Self::internal(StatusCode::INTERNAL_SERVER_ERROR, "the store failed")
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
            [(header::CONTENT_TYPE, PROBLEM_JSON)],
            body.to_string(),
        )
            .into_response();
        if let Some(head) = self.head {
            set_etag(&mut response, head);
        }
        response
    }
}

/// Answers a failure that axum answers itself, such as a body above the
/// limit, as a problem too.
pub(crate) async fn as_problem(response: Response) -> Response {
    let status = response.status();
    let is_problem = response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|value| value == PROBLEM_JSON);
    if !(status.is_client_error() || status.is_server_error()) || is_problem {
        return response;
    }
    let body = axum::body::to_bytes(response.into_body(), BODY_LIMIT)
        .await
        .unwrap_or_default();
    let detail = String::from_utf8_lossy(&body).into_owned();
    if status.is_server_error() {
        return Problem::internal(status, detail).into_response();
    }
    Problem::new(status, "invalid", detail).into_response()
}

/// The `ETag` of an operation, which `If-Match` names too.
pub(crate) fn etag(operation_id: &[u8]) -> String {
    format!("\"{}\"", hex::encode(operation_id))
}

pub(crate) fn set_etag(response: &mut Response, head: OperationId) {
    if let Ok(tag) = HeaderValue::try_from(etag(head.as_bytes())) {
        response.headers_mut().insert(header::ETAG, tag);
    }
}

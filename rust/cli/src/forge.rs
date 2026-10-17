//! The forge's REST API and `sync/v1`, as `jf` calls them. Every request
//! carries the trace of its command, as ADR 0005 decides, so no call names a
//! trace.

use std::fmt;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use jjforge_proto::sync::v1::Challenge;
use jjforge_proto::sync::v1::HashList;
use jjforge_proto::sync::v1::Head;
use jjforge_proto::sync::v1::ObjectBundle;
use jjforge_proto::sync::v1::PublishRequest;
use jjforge_proto::sync::v1::PublishResponse;
use jjforge_proto::sync::v1::Session;
use jjforge_proto::sync::v1::SessionRequest;
use jjforge_proto::sync::v1::Uploaded;
use jjforge_store::ObjectId;
use jjforge_store::OperationId;
use jjforge_store::Published;
use jjforge_store::SegmentId;
use jjforge_store::client::Fetch;
use jjforge_store::client::Transport;
use jjforge_store::client::TransportError;
use prost::Message;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::ETAG;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::IF_MATCH;
use serde::Deserialize;
use serde::Serialize;

use crate::credential::Credential;

/// The `Echo` schema of shared/openapi.yaml.
#[derive(Serialize, Deserialize)]
struct Echo {
    message: String,
}

/// The forge as one command calls it. `jf` sends no spans, so the server and
/// vcs continue the command's trace, and the command prints its ID for a
/// person to quote.
pub(crate) struct Client {
    endpoint: Url,
    http: reqwest::Client,
    trace: Trace,
}

impl fmt::Debug for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Client {
    /// Starts the command's trace. Clippy refuses any other HTTP client, so no
    /// request can leave without the trace.
    pub(crate) fn new(endpoint: Url) -> anyhow::Result<Self> {
        Self::with_trace(endpoint, Trace(u128::from_be_bytes(random()?)))
    }

    #[allow(clippy::disallowed_methods)]
    fn with_trace(endpoint: Url, trace: Trace) -> anyhow::Result<Self> {
        // reqwest needs one TLS provider per process. A second install, from
        // a test, fails and changes nothing.
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();

        let span = u64::from_be_bytes(random()?);
        let mut headers = HeaderMap::new();
        headers.insert(
            "traceparent",
            HeaderValue::try_from(format!("00-{trace}-{span:016x}-01"))?,
        );
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Self {
            endpoint,
            http,
            trace,
        })
    }

    /// The ID of the command's trace.
    pub(crate) fn trace(&self) -> String {
        self.trace.to_string()
    }

    /// Calls `POST /api/v1/echo` on the forge and returns the message it
    /// answered.
    pub(crate) async fn echo(&self, message: String) -> anyhow::Result<String> {
        let answer: Echo = self
            .http
            .post(self.endpoint.join("api/v1/echo")?)
            .json(&Echo { message })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(answer.message)
    }
}

const PROTOBUF: &str = "application/x-protobuf";

/// A failure the forge answered as `application/problem+json`.
#[derive(Debug, thiserror::Error)]
#[error("the forge answered {status}: {}", detail.as_deref().unwrap_or(code.as_deref().unwrap_or("no detail")))]
pub(crate) struct Problem {
    status: StatusCode,
    code: Option<String>,
    detail: Option<String>,
    /// The forge's head, from the `ETag` of a stale publish.
    head: Option<OperationId>,
}

#[derive(Deserialize)]
struct ProblemBody {
    code: Option<String>,
    detail: Option<String>,
}

/// One repository's `sync/v1`, as the store crate's `Transport` asks. jj-cli
/// runs commands on its own executor, so each call blocks on a runtime of
/// its own. With a credential, it opens a session on the first call and
/// opens another shortly before that one ends.
#[derive(Debug)]
pub(crate) struct SyncClient {
    client: Client,
    base: Url,
    credential: Option<Credential>,
    /// The session and when it ends, in seconds since the epoch.
    session: Mutex<Option<(String, i64)>>,
    runtime: tokio::runtime::Runtime,
}

/// How long before its end `jf` replaces a session, so no request carries one
/// that ends on the way.
const RENEW_BEFORE: i64 = 30;

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client({}, trace {})", self.endpoint, self.trace)
    }
}

impl SyncClient {
    /// Calls the repository at the clone address `base`, signed in with
    /// `credential` when there is one.
    pub(crate) fn new(base: Url, credential: Option<Credential>) -> anyhow::Result<Self> {
        Ok(Self {
            client: Client::new(base.clone())?,
            base,
            credential,
            session: Mutex::default(),
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?,
        })
    }

    fn url(&self, path: &str) -> Result<Url, TransportError> {
        let base = self.base.as_str().trim_end_matches('/');
        Ok(Url::parse(&format!("{base}/{path}"))?)
    }

    /// The address of a route of `sync/v1` itself, such as `session`, from
    /// a clone address `{host}/sync/v1/{org}/{repo}`.
    fn root(&self, route: &str) -> Result<Url, TransportError> {
        let base = self.base.as_str();
        let at = base
            .find("/sync/v1/")
            .ok_or_else(|| format!("{base} is not a clone address"))?;
        Ok(Url::parse(&format!("{}/sync/v1/{route}", &base[..at]))?)
    }

    /// The current session, opened or renewed when it has none or its
    /// session ends soon, or `None` without a credential.
    pub(crate) fn session(&self) -> Result<Option<String>, TransportError> {
        let Some(credential) = &self.credential else {
            return Ok(None);
        };
        let mut session = self
            .session
            .lock()
            .map_err(|_| "a session request panicked")?;
        if let Some((token, ends)) = &*session
            && *ends - RENEW_BEFORE > now()
        {
            return Ok(Some(token.clone()));
        }

        let challenge = self.client.http.get(self.root("challenge")?);
        let challenge: Challenge = decode(&self.runtime, self.answer(challenge)?)?;
        // The signature names the forge that `jf` connected to, so a forge
        // that relays another forge's challenge gains nothing.
        let origin = self.base.origin().ascii_serialization();
        let signed = [origin.as_bytes(), b"\n", &challenge.nonce].concat();
        let request = SessionRequest {
            certificate: credential.certificate()?,
            signature: credential.sign(&signed)?,
            nonce: challenge.nonce,
            origin,
        };
        let opened = self
            .client
            .http
            .post(self.root("session")?)
            .header(CONTENT_TYPE, PROTOBUF)
            .body(request.encode_to_vec());
        let opened: Session = decode(&self.runtime, self.answer(opened)?)?;
        *session = Some((opened.token.clone(), opened.expires_at));
        Ok(Some(opened.token))
    }

    fn send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response, TransportError> {
        let request = match self.session()? {
            Some(token) => request.header(AUTHORIZATION, format!("Bearer {token}")),
            None => request,
        };
        self.answer(request)
    }

    fn answer(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, TransportError> {
        self.runtime.block_on(async {
            let response = request.send().await?;
            if response.status().is_success() {
                return Ok(response);
            }
            Err(problem(response).await.into())
        })
    }

    fn post<M: Message + Default>(
        &self,
        path: &str,
        body: &impl Message,
    ) -> Result<M, TransportError> {
        let response = self.send(
            self.client
                .http
                .post(self.url(path)?)
                .header(CONTENT_TYPE, PROTOBUF)
                .body(body.encode_to_vec()),
        )?;
        decode(&self.runtime, response)
    }
}

async fn problem(response: reqwest::Response) -> Problem {
    let status = response.status();
    let head = response
        .headers()
        .get(ETAG)
        .and_then(|tag| tag.to_str().ok())
        .and_then(|tag| tag.trim_matches('"').parse().ok());
    let body: Option<ProblemBody> = response.json().await.ok();
    Problem {
        status,
        code: body.as_ref().and_then(|body| body.code.clone()),
        detail: body.and_then(|body| body.detail),
        head,
    }
}

fn hash_list(ids: &[ObjectId]) -> HashList {
    HashList {
        ids: ids.iter().map(ObjectId::to_vec).collect(),
    }
}

fn decode<M: Message + Default>(
    runtime: &tokio::runtime::Runtime,
    response: reqwest::Response,
) -> Result<M, TransportError> {
    let bytes = runtime.block_on(response.bytes())?;
    Ok(M::decode(bytes)?)
}

impl Fetch for SyncClient {
    fn fetch(&self, ids: &[ObjectId]) -> Result<Vec<Vec<u8>>, TransportError> {
        let bundle: ObjectBundle = self.post("objects/fetch", &hash_list(ids))?;
        Ok(bundle.objects)
    }
}

impl Transport for SyncClient {
    fn head(&self) -> Result<OperationId, TransportError> {
        let response = self.send(self.client.http.get(self.url("head")?))?;
        let head: Head = decode(&self.runtime, response)?;
        Ok(OperationId::try_from(head.operation_id.as_slice())?)
    }

    fn operations(&self, since: Option<OperationId>) -> Result<Vec<Vec<u8>>, TransportError> {
        let mut url = self.url("operations")?;
        if let Some(since) = since {
            url.query_pairs_mut()
                .append_pair("since", &since.to_string());
        }
        let response = self.send(self.client.http.get(url))?;
        let bundle: ObjectBundle = decode(&self.runtime, response)?;
        Ok(bundle.objects)
    }

    fn missing(&self, ids: &[ObjectId]) -> Result<Vec<ObjectId>, TransportError> {
        let missing: HashList = self.post("objects/missing", &hash_list(ids))?;
        Ok(missing
            .ids
            .iter()
            .map(|id| ObjectId::try_from(id.as_slice()))
            .collect::<Result<_, _>>()?)
    }

    fn upload(&self, objects: Vec<Vec<u8>>) -> Result<SegmentId, TransportError> {
        let uploaded: Uploaded = self.post("objects", &ObjectBundle { objects })?;
        Ok(SegmentId::try_from(uploaded.segment_id.as_slice())?)
    }

    /// Answers a stale head as `Published::Stale` with the head the forge
    /// names in its `ETag`, as #89 decides.
    fn publish(&self, request: &PublishRequest) -> Result<Published, TransportError> {
        let http = self
            .client
            .http
            .post(self.url("publish")?)
            .header(CONTENT_TYPE, PROTOBUF)
            .header(
                IF_MATCH,
                format!("\"{}\"", hex::encode(&request.expected_operation_id)),
            )
            .body(request.encode_to_vec());
        match self.send(http) {
            Ok(response) => {
                let published: PublishResponse = decode(&self.runtime, response)?;
                Ok(Published::Moved(OperationId::try_from(
                    published.operation_id.as_slice(),
                )?))
            }
            Err(err) => match err.downcast::<Problem>() {
                Ok(problem) if problem.status == StatusCode::PRECONDITION_FAILED => {
                    match problem.head {
                        Some(head) => Ok(Published::Stale(head)),
                        None => Err(problem),
                    }
                }
                Ok(problem) => Err(problem),
                Err(err) => Err(err),
            },
        }
    }
}

struct Trace(u128);

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

fn random<const N: usize>() -> anyhow::Result<[u8; N]> {
    let mut bytes = [0; N];
    rustls::crypto::ring::default_provider()
        .secure_random
        .fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("the system has no random numbers"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    use super::*;

    /// Answers one request with `body` and returns the request as text.
    async fn answer_once(listener: TcpListener, body: &str) -> String {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut chunk = [0; 1024];
        while !request.ends_with(b"}") {
            let n = socket.read(&mut chunk).await.unwrap();
            request.extend_from_slice(&chunk[..n]);
        }

        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        String::from_utf8(request).unwrap()
    }

    #[tokio::test]
    async fn echo_posts_the_message_to_the_api() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint: Url = format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let server = tokio::spawn(answer_once(listener, r#"{"message":"hi back"}"#));
        let client =
            Client::with_trace(endpoint, Trace(0x4bf9_2f35_77b3_4da6_a3ce_929d_0e0e_4736)).unwrap();

        let answer = client.echo("hi".to_owned()).await.unwrap();

        let request = server.await.unwrap();
        assert!(
            request.starts_with("POST /api/v1/echo HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(request.ends_with(r#"{"message":"hi"}"#), "{request}");
        assert!(
            request.contains("traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-"),
            "{request}"
        );
        assert_eq!(answer, "hi back");
    }
}

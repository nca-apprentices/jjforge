//! The forge's REST API and `sync/v1`, as `jf` calls them. Every request
//! carries the trace of its command, as ADR 0005 decides, so no call names a
//! trace.

use std::error::Error as StdError;
use std::fmt;

use jjforge_proto::sync::v1::HashList;
use jjforge_proto::sync::v1::Head;
use jjforge_proto::sync::v1::ObjectBundle;
use jjforge_proto::sync::v1::PublishRequest;
use jjforge_proto::sync::v1::PublishResponse;
use jjforge_proto::sync::v1::Uploaded;
use jjforge_store::Fetch;
use jjforge_store::Published;
use jjforge_store::Remote;
use jjforge_store::RemoteError;
use prost::Message;
use reqwest::StatusCode;
use reqwest::Url;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::IF_MATCH;
use serde::Deserialize;
use serde::Serialize;

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
}

#[derive(Deserialize)]
struct ProblemBody {
    code: Option<String>,
    detail: Option<String>,
}

/// One repository's `sync/v1`, as the store crate's `Remote` asks. jj-cli
/// runs commands on its own executor, so each call blocks on a runtime of
/// its own.
#[derive(Debug)]
pub(crate) struct SyncClient {
    client: Client,
    base: Url,
    token: Option<String>,
    runtime: tokio::runtime::Runtime,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Client({}, trace {})", self.endpoint, self.trace)
    }
}

impl SyncClient {
    /// Calls the repository at the clone address `base`, with the token of
    /// `JJFORGE_TOKEN` when one is set.
    pub(crate) fn new(base: Url, token: Option<String>) -> anyhow::Result<Self> {
        Ok(Self {
            client: Client::new(base.clone())?,
            base,
            token,
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?,
        })
    }

    fn url(&self, path: &str) -> Result<Url, RemoteError> {
        let base = self.base.as_str().trim_end_matches('/');
        Ok(Url::parse(&format!("{base}/{path}"))?)
    }

    fn send(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response, RemoteError> {
        let request = match &self.token {
            Some(token) => request.header(AUTHORIZATION, format!("Bearer {token}")),
            None => request,
        };
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
    ) -> Result<M, RemoteError> {
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
    let body: Option<ProblemBody> = response.json().await.ok();
    Problem {
        status,
        code: body.as_ref().and_then(|body| body.code.clone()),
        detail: body.and_then(|body| body.detail),
    }
}

fn decode<M: Message + Default>(
    runtime: &tokio::runtime::Runtime,
    response: reqwest::Response,
) -> Result<M, RemoteError> {
    let bytes = runtime.block_on(response.bytes())?;
    Ok(M::decode(bytes)?)
}

impl Fetch for SyncClient {
    fn fetch(&self, ids: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, Box<dyn StdError + Send + Sync>> {
        let bundle: ObjectBundle = self.post("objects/fetch", &HashList { ids: ids.to_vec() })?;
        Ok(bundle.objects)
    }
}

impl Remote for SyncClient {
    fn head(&self) -> Result<Vec<u8>, RemoteError> {
        let response = self.send(self.client.http.get(self.url("head")?))?;
        let head: Head = decode(&self.runtime, response)?;
        Ok(head.operation_id)
    }

    fn operations(&self, since: Option<&[u8]>) -> Result<Vec<Vec<u8>>, RemoteError> {
        let mut url = self.url("operations")?;
        if let Some(since) = since {
            url.query_pairs_mut()
                .append_pair("since", &hex::encode(since));
        }
        let response = self.send(self.client.http.get(url))?;
        let bundle: ObjectBundle = decode(&self.runtime, response)?;
        Ok(bundle.objects)
    }

    fn missing(&self, ids: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, RemoteError> {
        let missing: HashList = self.post("objects/missing", &HashList { ids })?;
        Ok(missing.ids)
    }

    fn upload(&self, objects: Vec<Vec<u8>>) -> Result<Vec<u8>, RemoteError> {
        let uploaded: Uploaded = self.post("objects", &ObjectBundle { objects })?;
        Ok(uploaded.segment_id)
    }

    /// Answers a stale head as `Published::Stale` with the head the forge
    /// names in its `ETag`, as #89 decides.
    fn publish(&self, request: &PublishRequest) -> Result<Published, RemoteError> {
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
                Ok(Published::Moved(published.operation_id))
            }
            Err(err) => match err.downcast::<Problem>() {
                Ok(problem) if problem.status == StatusCode::PRECONDITION_FAILED => {
                    Ok(Published::Stale(jjforge_store::Head {
                        operation_id: vec![],
                    }))
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

//! The forge's REST API, as `jf` calls it. Every request carries the trace of
//! its command, as ADR 0005 decides, so no call names a trace.

use std::fmt;

use reqwest::Url;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
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

//! `jf echo <message>` sends the message through the forge's REST API and
//! prints the answer. It proves the client reaches the server, which passes
//! the message through vcsd, and nothing more.

use clap::Parser;
use clap::Subcommand;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;

/// The jjforge command-line client. Bad usage exits with status 2, as
/// shared/docs/cli.md describes.
#[derive(Parser)]
#[command(name = "jf", version, about)]
struct Cli {
    /// The forge to talk to.
    #[arg(
        long,
        env = "JJFORGE_ENDPOINT",
        default_value = "http://localhost:8080",
        global = true
    )]
    endpoint: Url,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sends a message through the forge and prints the answer.
    Echo {
        /// The message, joined with spaces.
        #[arg(required = true)]
        words: Vec<String>,
    },
}

/// The `Echo` schema of shared/openapi.yaml.
#[derive(Serialize, Deserialize)]
struct Echo {
    message: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let Command::Echo { words } = cli.command;

    println!("{}", echo(&cli.endpoint, words.join(" ")).await?);
    Ok(())
}

/// Calls `POST /api/v1/echo` on the forge and returns the message it answered.
async fn echo(endpoint: &Url, message: String) -> anyhow::Result<String> {
    // reqwest needs one TLS provider per process. A second install, from a
    // test, fails and changes nothing.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let answer: Echo = reqwest::Client::new()
        .post(endpoint.join("api/v1/echo")?)
        .json(&Echo { message })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(answer.message)
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    use super::*;

    #[test]
    fn echo_needs_a_message() {
        assert!(Cli::try_parse_from(["jf", "echo"]).is_err());
    }

    #[test]
    fn endpoint_must_be_an_address() {
        assert!(Cli::try_parse_from(["jf", "--endpoint", "not an address", "echo", "hi"]).is_err());
    }

    #[test]
    fn echo_joins_the_words() {
        let cli = Cli::try_parse_from(["jf", "echo", "hello", "there"]).unwrap();
        let Command::Echo { words } = cli.command;

        assert_eq!(words.join(" "), "hello there");
    }

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

        let answer = echo(&endpoint, "hi".to_owned()).await.unwrap();

        let request = server.await.unwrap();
        assert!(
            request.starts_with("POST /api/v1/echo HTTP/1.1\r\n"),
            "{request}"
        );
        assert!(request.ends_with(r#"{"message":"hi"}"#), "{request}");
        assert_eq!(answer, "hi back");
    }
}

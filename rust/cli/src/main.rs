//! `jf` is jj, built on jj-cli's `CliRunner`, with the forge's commands added,
//! as ADR 0001 decides. `jf echo <message>` sends the message through the
//! forge's REST API and prints the answer. It proves the client reaches the
//! server, which passes the message through vcs, and nothing more.

use std::io::Write as _;
use std::process::ExitCode;

use clap::Args;
use clap::FromArgMatches as _;
use clap::Subcommand;
use jj_cli::cli_util::CliRunner;
use jj_cli::cli_util::CommandHelper;
use jj_cli::command_error::CommandError;
use jj_cli::command_error::cli_error;
use jj_cli::command_error::user_error;
use jj_cli::ui::Ui;
use reqwest::Url;
use serde::Deserialize;
use serde::Serialize;

/// The forge to talk to, an option of every command.
#[derive(Args)]
struct Forge {
    /// The forge to talk to.
    #[arg(
        long,
        env = "JJFORGE_ENDPOINT",
        default_value = "http://localhost:8080",
        global = true
    )]
    endpoint: Url,
}

/// The commands `jf` adds to jj's. Bad usage exits with status 2, as
/// shared/docs/cli.md describes.
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

fn main() -> ExitCode {
    // jj-cli hands a global option to its own callback, before the command
    // runs, so `run` reads `Forge` from the matches instead. The about text
    // comes last, because each addition replaces it with its doc comment.
    CliRunner::init()
        .add_global_args(|_: &mut Ui, _: Forge| Ok(()))
        .add_subcommand(run)
        .name("jf")
        .about("The jjforge command-line client")
        .version(env!("CARGO_PKG_VERSION"))
        .run()
        .into()
}

async fn run(ui: &mut Ui, helper: &CommandHelper, command: Command) -> Result<(), CommandError> {
    let forge = Forge::from_arg_matches(helper.matches()).map_err(cli_error)?;
    let Command::Echo { words } = command;

    // jj-cli runs commands on its own executor, and reqwest needs tokio.
    let answer = tokio::runtime::Runtime::new()?
        .block_on(echo(&forge.endpoint, words.join(" ")))
        .map_err(user_error)?;
    writeln!(ui.stdout(), "{answer}")?;
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

    /// Parses `args` as jj-cli does for the options and commands `jf` adds.
    fn parse(args: &[&str]) -> Result<(Forge, Command), clap::Error> {
        let matches = Command::augment_subcommands(Forge::augment_args(clap::Command::new("jf")))
            .try_get_matches_from(args)?;
        Ok((
            Forge::from_arg_matches(&matches)?,
            Command::from_arg_matches(&matches)?,
        ))
    }

    #[test]
    fn echo_needs_a_message() {
        assert!(parse(&["jf", "echo"]).is_err());
    }

    #[test]
    fn endpoint_must_be_an_address() {
        assert!(parse(&["jf", "--endpoint", "not an address", "echo", "hi"]).is_err());
    }

    #[test]
    fn echo_joins_the_words() {
        let (_, Command::Echo { words }) = parse(&["jf", "echo", "hello", "there"]).unwrap();

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

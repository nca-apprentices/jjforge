//! `jf echo <message>` sends the message to vcsd and prints the answer.
//! It proves the client can reach vcsd over `echo/v1` and nothing more.

use clap::Parser;
use clap::Subcommand;
use jjforge_proto::echo::v1::EchoRequest;
use jjforge_proto::echo::v1::echo_service_client::EchoServiceClient;
use tonic::transport::Channel;
use tonic::transport::ClientTlsConfig;
use tonic::transport::Uri;

/// The jjforge command-line client. Bad usage exits with status 2, as
/// shared/docs/cli.md describes.
#[derive(Parser)]
#[command(name = "jf", version, about)]
struct Cli {
    /// The forge to talk to.
    #[arg(
        long,
        env = "JJFORGE_ENDPOINT",
        default_value = "http://localhost:50052",
        global = true
    )]
    endpoint: Uri,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sends a message through vcsd and prints the answer.
    Echo {
        /// The message, joined with spaces.
        #[arg(required = true)]
        words: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let Command::Echo { words } = cli.command;

    let channel = Channel::builder(cli.endpoint)
        .tls_config(ClientTlsConfig::new().with_native_roots())?
        .connect()
        .await?;
    let mut client = EchoServiceClient::new(channel);
    let response = client
        .echo(EchoRequest {
            message: words.join(" "),
        })
        .await?;

    println!("{}", response.into_inner().message);
    Ok(())
}

#[cfg(test)]
mod tests {
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
}

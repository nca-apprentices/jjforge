//! `jjforge echo <message>` sends the message to vcsd and prints the answer.
//! It proves the client can reach vcsd over `echo/v1` and nothing more.

use anyhow::bail;
use jjforge_proto::echo::v1::EchoRequest;
use jjforge_proto::echo::v1::echo_service_client::EchoServiceClient;
use tonic::transport::Channel;
use tonic::transport::ClientTlsConfig;

const DEFAULT_ENDPOINT: &str = "http://localhost:50052";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(("echo", words)) = args.split_first().map(|(c, w)| (c.as_str(), w)) else {
        bail!("usage: jjforge echo <message>");
    };

    let endpoint =
        std::env::var("JJFORGE_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_owned());
    let channel = Channel::from_shared(endpoint)?
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

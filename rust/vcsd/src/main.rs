//! vcsd serves `echo/v1` on one port. The client and the server both call it,
//! which is all it has to prove for now.

use std::net::SocketAddr;

use clap::Parser;
use jjforge_proto::echo::v1::EchoRequest;
use jjforge_proto::echo::v1::EchoResponse;
use jjforge_proto::echo::v1::echo_service_server::EchoService;
use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
use tonic::Request;
use tonic::Response;
use tonic::Status;

/// vcsd's settings, from flags or the environment. An invalid value stops
/// vcsd at startup with a usage message.
#[derive(Parser)]
#[command(version, about)]
struct Config {
    /// The address to serve gRPC on.
    #[arg(long, env = "VCSD_ADDR", default_value = "0.0.0.0:50052")]
    addr: SocketAddr,
}

struct Echo;

#[tonic::async_trait]
impl EchoService for Echo {
    async fn echo(&self, request: Request<EchoRequest>) -> Result<Response<EchoResponse>, Status> {
        Ok(Response::new(EchoResponse {
            message: request.into_inner().message,
        }))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();

    println!("vcsd listening on {}", config.addr);
    tonic::transport::Server::builder()
        .add_service(EchoServiceServer::new(Echo))
        .serve_with_shutdown(config.addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_returns_the_message() {
        let request = Request::new(EchoRequest {
            message: "hello".to_owned(),
        });

        let response = Echo.echo(request).await.unwrap();

        assert_eq!(response.into_inner().message, "hello");
    }

    #[test]
    fn config_refuses_an_invalid_address() {
        assert!(Config::try_parse_from(["vcsd", "--addr", "nowhere"]).is_err());
    }

    #[test]
    fn config_reads_an_address() {
        let config = Config::try_parse_from(["vcsd", "--addr", "127.0.0.1:1"]).unwrap();

        assert_eq!(config.addr.port(), 1);
    }
}

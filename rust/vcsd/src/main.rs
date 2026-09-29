//! vcsd serves `echo/v1` on one port. The client and the server both call it,
//! which is all it has to prove for now.

use std::net::SocketAddr;

use jjforge_proto::echo::v1::EchoRequest;
use jjforge_proto::echo::v1::EchoResponse;
use jjforge_proto::echo::v1::echo_service_server::EchoService;
use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
use tonic::Request;
use tonic::Response;
use tonic::Status;

const DEFAULT_ADDR: &str = "0.0.0.0:50052";

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
    let addr: SocketAddr = std::env::var("VCSD_ADDR")
        .unwrap_or_else(|_| DEFAULT_ADDR.to_owned())
        .parse()?;

    println!("vcsd listening on {addr}");
    tonic::transport::Server::builder()
        .add_service(EchoServiceServer::new(Echo))
        .serve_with_shutdown(addr, async {
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
}

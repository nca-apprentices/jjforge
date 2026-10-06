//! vcs serves `echo/v1` on one port. The server calls it, which is all it
//! has to prove for now. Prometheus metrics are served on a second port.

mod config;
mod echo;
mod metrics;

use clap::Parser;
use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;

use crate::config::Config;
use crate::echo::Echo;
use crate::metrics::Meter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    metrics::install(config.metrics_addr)?;

    println!(
        "vcs listening on {}, metrics on {}",
        config.addr, config.metrics_addr
    );
    tonic::transport::Server::builder()
        .layer(Meter)
        .add_service(EchoServiceServer::new(Echo))
        .serve_with_shutdown(config.addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    Ok(())
}

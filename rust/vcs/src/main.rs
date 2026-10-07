//! vcs serves `echo/v1` on one port. The server calls it, which is all it
//! has to prove for now. Prometheus metrics are served on a second port.

// A line written past `tracing` reaches no log store as JSON and carries no
// trace, as ADR 0005 decides.
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod config;
mod echo;
mod metrics;
mod telemetry;

use clap::Parser;
use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
use tonic_tracing_opentelemetry::middleware::server::OtelGrpcLayer;

use crate::config::Config;
use crate::echo::Echo;
use crate::metrics::Meter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    let traces = telemetry::install(&config.log, config.traces_endpoint.as_ref())?;
    metrics::install(config.metrics_addr)?;

    tracing::info!(addr = %config.addr, metrics_addr = %config.metrics_addr, "vcs listening");
    // The trace layer wraps every service added here, so none needs code of
    // its own to be traced.
    tonic::transport::Server::builder()
        .layer(Meter)
        .layer(OtelGrpcLayer::default())
        .add_service(EchoServiceServer::new(Echo))
        .serve_with_shutdown(config.addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    traces.shutdown()?;
    Ok(())
}

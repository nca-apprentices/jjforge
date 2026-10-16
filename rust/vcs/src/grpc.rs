//! The gRPC services that the server calls: `echo/v1`, and the
//! `RepoService` of `source/v1` when vcs has a store.

mod echo;
mod repos;

use std::net::SocketAddr;

use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
use jjforge_proto::source::v1::repo_service_server::RepoServiceServer;
use tonic_tracing_opentelemetry::middleware::server::OtelGrpcLayer;

use crate::gate::Gate;
use crate::metrics::Meter;

/// The telemetry tests trace and meter a call to it.
#[cfg(test)]
pub(crate) use echo::Echo;

/// Serves gRPC on `addr` until the process gets Ctrl-C.
pub(crate) async fn serve(addr: SocketAddr, gate: Option<Gate>) -> anyhow::Result<()> {
    tracing::info!(%addr, "gRPC listening");
    // The trace layer wraps every service added here, so none needs code of
    // its own to be traced.
    tonic::transport::Server::builder()
        .layer(Meter)
        .layer(OtelGrpcLayer::default())
        .add_service(EchoServiceServer::new(echo::Echo))
        .add_optional_service(gate.map(|gate| RepoServiceServer::new(repos::Service(gate))))
        .serve_with_shutdown(addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

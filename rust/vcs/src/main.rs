//! vcs serves gRPC on one port: `echo/v1`, and the `RepoService` of
//! `source/v1` that creates and deletes repositories. It serves `sync/v1`
//! to `jf` over HTTP on a second port, and Prometheus metrics on a third.

// A line written past `tracing` reaches no log store as JSON and carries no
// trace, as ADR 0005 decides.
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod auth;
mod config;
mod echo;
mod metrics;
mod repos;
mod sync;
mod telemetry;

use std::sync::Arc;

use clap::Parser;
use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
use jjforge_proto::source::v1::repo_service_server::RepoServiceServer;
use jjforge_store::Repos;
use tonic_tracing_opentelemetry::middleware::server::OtelGrpcLayer;

use crate::auth::Verifier;
use crate::config::Config;
use crate::echo::Echo;
use crate::metrics::Meter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    let traces = telemetry::install(&config.log, config.traces_endpoint.as_ref())?;
    metrics::install(config.metrics_addr)?;

    // The S3 client and the key set client share reqwest's TLS, which
    // leaves the provider to the binary. A second install changes nothing.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let repos = match &config.store {
        Some(url) => {
            let cache = config
                .cache
                .clone()
                .unwrap_or_else(|| std::env::temp_dir().join("vcs"));
            let verifier = Arc::new(Verifier::new(config.token_issuers.clone())?);
            Some((Arc::new(Repos::open(url, &cache)?), verifier))
        }
        None => None,
    };

    if let Some((repos, verifier)) = &repos {
        // Meter labels gRPC methods, and a sync/v1 path holds IDs, which never
        // label a metric, as ADR 0005 decides. The span of each request
        // records the route instead.
        let router = sync::router(repos.clone(), verifier.clone())
            .layer(axum::middleware::from_fn(telemetry::trace_http));
        let listener = tokio::net::TcpListener::bind(config.sync_addr).await?;
        tracing::info!(sync_addr = %config.sync_addr, "sync/v1 listening");
        tokio::spawn(async move {
            if let Err(err) = axum::serve(listener, router).await {
                tracing::error!(error = %err, "sync/v1 stopped");
            }
        });
    }

    tracing::info!(addr = %config.addr, metrics_addr = %config.metrics_addr, "vcs listening");
    // The trace layer wraps every service added here, so none needs code of
    // its own to be traced.
    let repo_service =
        repos.map(|(repos, verifier)| RepoServiceServer::new(repos::Service { repos, verifier }));
    tonic::transport::Server::builder()
        .layer(Meter)
        .layer(OtelGrpcLayer::default())
        .add_service(EchoServiceServer::new(Echo))
        .add_optional_service(repo_service)
        .serve_with_shutdown(config.addr, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    traces.shutdown()?;
    Ok(())
}

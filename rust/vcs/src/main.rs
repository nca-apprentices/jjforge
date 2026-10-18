//! vcs serves gRPC on one port: `echo/v1`, and the `RepoService` of
//! `source/v1` that creates and deletes repositories. It serves `sync/v1`
//! to `jf` over HTTP on a second port, and Prometheus metrics on a third.
//!
//! ```text
//! server ──gRPC──▶ grpc ─┐
//!                        ├─▶ gate ──▶ store crate's Repos ──▶ object store
//! jf ───sync/v1──▶ sync ─┘     │
//!                              └─ auth looks up the principal
//! ```

// A line written past `tracing` reaches no log store as JSON and carries no
// trace, as ADR 0005 decides.
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod auth;
mod config;
mod gate;
mod grpc;
mod metrics;
mod sync;
mod telemetry;

use clap::Parser;
use jjforge_store::server::Repos;

use crate::auth::Principals;
use crate::config::Config;
use crate::gate::Gate;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    let traces = telemetry::install(&config.log, config.traces_endpoint.as_ref())?;
    metrics::install(config.metrics_addr)?;

    // The S3 client uses reqwest's TLS, which leaves the provider to the
    // binary. A second install changes nothing.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    // Without a store, vcs serves only `echo/v1`.
    let gate = match &config.store {
        Some(url) => {
            let cache = config
                .cache
                .clone()
                .unwrap_or_else(|| std::env::temp_dir().join("vcs"));
            let repos = Repos::open(url, &cache)?;
            let principals = if let Some(path) = &config.principals {
                Principals::read(path)?
            } else {
                tracing::warn!("no principals, so vcs refuses every request");
                Principals::default()
            };
            Some(Gate::new(repos, principals))
        }
        None => None,
    };
    if let Some(gate) = &gate {
        sync::serve(config.sync_addr, gate.clone()).await?;
    }
    tracing::info!(metrics_addr = %config.metrics_addr, "metrics listening");
    grpc::serve(config.addr, gate).await?;

    traces.shutdown()?;
    Ok(())
}

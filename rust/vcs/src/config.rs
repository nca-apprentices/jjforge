//! vcs's settings, from flags or the environment.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use http::Uri;
use tracing_subscriber::EnvFilter;
use url::Url;

use crate::auth::Issuer;

/// vcs's settings. An invalid value stops vcs at startup with a usage
/// message.
#[derive(Parser)]
#[command(version, about)]
pub(crate) struct Config {
    /// The address to serve gRPC on.
    #[arg(long, env = "VCS_ADDR", default_value = "0.0.0.0:50052")]
    pub(crate) addr: SocketAddr,

    /// The address to serve `sync/v1` on, for `jf`.
    #[arg(long, env = "VCS_SYNC_ADDR", default_value = "0.0.0.0:8081")]
    pub(crate) sync_addr: SocketAddr,

    /// The object store that holds the repositories, as ADR 0002 decides:
    /// `s3://{bucket}` with the `AWS_*` environment, or `file://{dir}`.
    /// Without it, vcs serves `echo/v1` only.
    #[arg(long, env = "VCS_STORE")]
    pub(crate) store: Option<Url>,

    /// Where vcs keeps jj's index of each repository, which it rebuilds
    /// when the directory is gone. The system's temporary directory by
    /// default.
    #[arg(long, env = "VCS_CACHE")]
    pub(crate) cache: Option<PathBuf>,

    /// The issuers whose tokens vcs trusts, separated by commas, each with
    /// the organization it signs for, such as `acme=https://id.acme.example`.
    /// vcs finds their keys through the discovery of `OpenID` Connect.
    #[arg(long, env = "VCS_TOKEN_ISSUERS", value_delimiter = ',')]
    pub(crate) token_issuers: Vec<Issuer>,

    /// The address to serve Prometheus metrics on, at `/metrics`.
    #[arg(long, env = "VCS_METRICS_ADDR", default_value = "0.0.0.0:9090")]
    pub(crate) metrics_addr: SocketAddr,

    /// Where to send traces over OTLP/HTTP, such as
    /// `http://telemetry.observability.svc:4318/v1/traces`. Without it, vcs
    /// sends none.
    #[arg(long, env = "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")]
    pub(crate) traces_endpoint: Option<Uri>,

    /// Which events to log, as a `tracing` filter such as `info,vcs=debug`.
    /// jj-lib logs every index lock at info, so the default quiets it.
    #[arg(long, env = "VCS_LOG", default_value = "info,jj_lib=warn", value_parser = log_filter)]
    pub(crate) log: String,
}

fn log_filter(filter: &str) -> Result<String, String> {
    EnvFilter::try_new(filter).map_err(|error| error.to_string())?;
    Ok(filter.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_refuses_an_invalid_address() {
        assert!(Config::try_parse_from(["vcs", "--addr", "nowhere"]).is_err());
    }

    #[test]
    fn config_reads_an_address() {
        let config = Config::try_parse_from(["vcs", "--addr", "127.0.0.1:1"]).unwrap();

        assert_eq!(config.addr.port(), 1);
    }

    #[test]
    fn config_refuses_an_invalid_metrics_address() {
        assert!(Config::try_parse_from(["vcs", "--metrics-addr", "nowhere"]).is_err());
    }

    #[test]
    fn config_refuses_an_invalid_log_filter() {
        assert!(Config::try_parse_from(["vcs", "--log", "vcs=loud"]).is_err());
    }

    #[test]
    fn config_refuses_an_invalid_traces_endpoint() {
        assert!(Config::try_parse_from(["vcs", "--traces-endpoint", "not a url"]).is_err());
    }

    #[test]
    fn config_reads_the_store_and_its_issuers() {
        let config = Config::try_parse_from([
            "vcs",
            "--store",
            "s3://jjforge",
            "--token-issuers",
            "acme=http://oidc:8080/acme,other=http://oidc:8080/other",
        ])
        .unwrap();

        assert_eq!(config.store.unwrap().as_str(), "s3://jjforge");
        assert_eq!(config.token_issuers.len(), 2);
    }

    #[test]
    fn config_reads_a_metrics_address() {
        let config = Config::try_parse_from(["vcs", "--metrics-addr", "127.0.0.1:2"]).unwrap();

        assert_eq!(config.metrics_addr.port(), 2);
    }
}

//! vcs serves `echo/v1` on one port. The server calls it, which is all it
//! has to prove for now. Prometheus metrics are served on a second port.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Instant;

use clap::Parser;
use jjforge_proto::echo::v1::EchoRequest;
use jjforge_proto::echo::v1::EchoResponse;
use jjforge_proto::echo::v1::echo_service_server::EchoService;
use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
use metrics_exporter_prometheus::Matcher;
use metrics_exporter_prometheus::PrometheusBuilder;
use tonic::Request;
use tonic::Response;
use tonic::Status;
use tower::Service;

const REQUESTS: &str = "vcs_grpc_requests_total";
const DURATION: &str = "vcs_grpc_request_duration_seconds";
const DURATION_BUCKETS: &[f64] = &[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0];

/// The gRPC status tonic answers a path no service implements with.
const UNIMPLEMENTED: &str = "12";

/// vcs's settings, from flags or the environment. An invalid value stops
/// vcs at startup with a usage message.
#[derive(Parser)]
#[command(version, about)]
struct Config {
    /// The address to serve gRPC on.
    #[arg(long, env = "VCS_ADDR", default_value = "0.0.0.0:50052")]
    addr: SocketAddr,

    /// The address to serve Prometheus metrics on, at `/metrics`.
    #[arg(long, env = "VCS_METRICS_ADDR", default_value = "0.0.0.0:9090")]
    metrics_addr: SocketAddr,
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

/// Counts and times every gRPC call by its method, the request path such as
/// `/echo.v1.EchoService/Echo`. An unimplemented path counts as `unknown`
/// rather than as a label value of its own, so a caller can't grow the label
/// set without end.
#[derive(Clone)]
struct Metered<S>(S);

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for Metered<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, request: http::Request<ReqBody>) -> Self::Future {
        let path = request.uri().path().to_owned();
        let start = Instant::now();
        let response = self.0.call(request);

        Box::pin(async move {
            let response = response.await;
            let unimplemented = response.as_ref().is_ok_and(|response| {
                response
                    .headers()
                    .get("grpc-status")
                    .is_some_and(|status| status == UNIMPLEMENTED)
            });
            let method = if unimplemented {
                "unknown".to_owned()
            } else {
                path
            };

            metrics::counter!(REQUESTS, "method" => method.clone()).increment(1);
            metrics::histogram!(DURATION, "method" => method).record(start.elapsed().as_secs_f64());
            response
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::parse();

    // Without buckets the exporter renders a summary, which can't be
    // aggregated across pods.
    PrometheusBuilder::new()
        .with_http_listener(config.metrics_addr)
        .set_buckets_for_metric(Matcher::Full(DURATION.to_owned()), DURATION_BUCKETS)?
        .install()?;

    println!(
        "vcs listening on {}, metrics on {}",
        config.addr, config.metrics_addr
    );
    tonic::transport::Server::builder()
        .layer(tower::layer::layer_fn(Metered))
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
    fn config_reads_a_metrics_address() {
        let config = Config::try_parse_from(["vcs", "--metrics-addr", "127.0.0.1:2"]).unwrap();

        assert_eq!(config.metrics_addr.port(), 2);
    }

    #[test]
    fn metered_labels_calls_by_method() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut service = Metered(EchoServiceServer::new(Echo));

        metrics::with_local_recorder(&recorder, || {
            runtime.block_on(async {
                for path in ["/echo.v1.EchoService/Echo", "/echo.v1.EchoService/Invented"] {
                    let request = http::Request::post(path)
                        .header("content-type", "application/grpc")
                        .body(tonic::body::Body::empty())
                        .unwrap();
                    service.call(request).await.unwrap();
                }
            });
        });
        let rendered = recorder.handle().render();

        assert!(
            rendered.contains(r#"vcs_grpc_requests_total{method="/echo.v1.EchoService/Echo"} 1"#)
        );
        assert!(rendered.contains(r#"vcs_grpc_requests_total{method="unknown"} 1"#));
        assert!(!rendered.contains("Invented"));
    }
}

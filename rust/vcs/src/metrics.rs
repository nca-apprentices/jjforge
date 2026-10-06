//! Prometheus metrics of every gRPC call, served on a port of their own.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Instant;

use metrics_exporter_prometheus::Matcher;
use metrics_exporter_prometheus::PrometheusBuilder;
use tower::Layer;
use tower::Service;

const REQUESTS: &str = "vcs_grpc_requests_total";
const DURATION: &str = "vcs_grpc_request_duration_seconds";
const DURATION_BUCKETS: &[f64] = &[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0];

/// The gRPC status tonic answers a path no service implements with.
const UNIMPLEMENTED: &str = "12";

/// Serves the metrics at `/metrics` on `addr`.
pub(crate) fn install(addr: SocketAddr) -> anyhow::Result<()> {
    // Without buckets the exporter renders a summary, which can't be
    // aggregated across pods.
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .set_buckets_for_metric(Matcher::Full(DURATION.to_owned()), DURATION_BUCKETS)?
        .install()?;
    Ok(())
}

/// Wraps each gRPC service in [`Metered`].
#[derive(Clone)]
pub(crate) struct Meter;

impl<S> Layer<S> for Meter {
    type Service = Metered<S>;

    fn layer(&self, inner: S) -> Metered<S> {
        Metered(inner)
    }
}

/// Counts and times every gRPC call by its method, the request path such as
/// `/echo.v1.EchoService/Echo`. An unimplemented path counts as `unknown`
/// rather than as a label value of its own, so a caller can't grow the label
/// set without end.
#[derive(Clone)]
pub(crate) struct Metered<S>(S);

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

#[cfg(test)]
mod tests {
    use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;

    use super::*;
    use crate::echo::Echo;

    #[test]
    fn metered_labels_calls_by_method() {
        let recorder = PrometheusBuilder::new().build_recorder();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut service = Meter.layer(EchoServiceServer::new(Echo));

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

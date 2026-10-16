//! `echo/v1`, which the server calls to prove it reaches vcs.

use jjforge_proto::echo::v1::EchoRequest;
use jjforge_proto::echo::v1::EchoResponse;
use jjforge_proto::echo::v1::echo_service_server::EchoService;
use tonic::Request;
use tonic::Response;
use tonic::Status;

pub(crate) struct Echo;

#[tonic::async_trait]
impl EchoService for Echo {
    /// A span of its own under the call's, so the trace shows the work that
    /// vcs does apart from the gRPC call around it.
    #[tracing::instrument(skip_all)]
    async fn echo(&self, request: Request<EchoRequest>) -> Result<Response<EchoResponse>, Status> {
        Ok(Response::new(EchoResponse {
            message: request.into_inner().message,
        }))
    }
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

    #[tokio::test]
    async fn echo_records_a_span_of_its_own() {
        use opentelemetry::trace::TracerProvider;
        use opentelemetry_sdk::trace::InMemorySpanExporter;
        use opentelemetry_sdk::trace::SdkTracerProvider;
        use tracing::instrument::WithSubscriber;
        use tracing_subscriber::layer::SubscriberExt;

        let spans = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(spans.clone())
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
        let request = Request::new(EchoRequest {
            message: "hi".to_owned(),
        });

        Echo.echo(request)
            .with_subscriber(subscriber)
            .await
            .unwrap();

        let names: Vec<_> = spans
            .get_finished_spans()
            .unwrap()
            .into_iter()
            .map(|span| span.name)
            .collect();
        assert_eq!(names, ["echo"]);
    }
}

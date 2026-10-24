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
    /// vcs does apart from the gRPC call around it. It records the message's
    /// length, never the message, which may be personal data.
    #[tracing::instrument(skip_all, fields(jjforge.echo.bytes = request.get_ref().message.len()))]
    async fn echo(&self, request: Request<EchoRequest>) -> Result<Response<EchoResponse>, Status> {
        Ok(Response::new(EchoResponse {
            message: request.into_inner().message,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::attribute;
    use crate::telemetry::finished_spans;

    #[tokio::test]
    async fn echo_returns_the_message() {
        let request = Request::new(EchoRequest {
            message: "hello".to_owned(),
        });

        let response = Echo.echo(request).await.unwrap();

        assert_eq!(response.into_inner().message, "hello");
    }

    #[tokio::test]
    async fn echo_records_what_it_did_on_a_span_of_its_own() {
        let request = Request::new(EchoRequest {
            message: "hi".to_owned(),
        });

        let (_, spans) = finished_spans(Echo.echo(request)).await;

        let names: Vec<_> = spans.iter().map(|span| span.name.to_string()).collect();
        assert_eq!(names, ["echo"]);
        assert_eq!(
            attribute(&spans, "jjforge.echo.bytes").as_deref(),
            Some("2")
        );
    }
}

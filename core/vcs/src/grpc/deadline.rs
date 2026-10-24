//! The deadline of every gRPC call, as ADR 0006 decides.

use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use tonic::Status;
use tonic::body::Body;
use tower::Layer;
use tower::Service;

/// Wraps each gRPC service in [`Timed`] with a deadline.
#[derive(Clone)]
pub(super) struct Deadline(pub(super) Duration);

impl<S> Layer<S> for Deadline {
    type Service = Timed<S>;

    fn layer(&self, inner: S) -> Timed<S> {
        Timed(inner, self.0)
    }
}

/// Answers `DEADLINE_EXCEEDED` when a handler runs past its deadline, which
/// the telemetry counts as a failure of vcs. tonic's own timeout answers
/// `CANCELLED`, which it would count as the caller's. A stream runs on after
/// its handler returns, so the deadline never ends a watch.
#[derive(Clone)]
pub(super) struct Timed<S>(S, Duration);

impl<S, ReqBody> Service<http::Request<ReqBody>> for Timed<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<Body>>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, request: http::Request<ReqBody>) -> Self::Future {
        let (response, deadline) = (self.0.call(request), self.1);
        Box::pin(async move {
            match tokio::time::timeout(deadline, response).await {
                Ok(response) => response,
                Err(_) => {
                    Ok(Status::deadline_exceeded("the call ran past its deadline").into_http())
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_call_past_its_deadline_answers_deadline_exceeded() {
        let slow = tower::service_fn(|_: http::Request<()>| async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok::<_, std::convert::Infallible>(http::Response::new(Body::default()))
        });
        let mut service = Deadline(Duration::from_millis(10)).layer(slow);

        let response = service.call(http::Request::new(())).await.unwrap();

        assert_eq!(response.headers()["grpc-status"], "4");
    }
}

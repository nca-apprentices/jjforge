//! Logs and traces, as ADR 0005 decides. A log line is one JSON object on
//! stdout with the trace that wrote it. Spans go to an OTLP endpoint when one
//! is set, and a caller's `traceparent` continues either way.

use std::fmt;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::Context;
use std::task::Poll;
use std::time::Instant;

use axum::extract::MatchedPath;
use axum::middleware::Next;
use http::Uri;
use opentelemetry::KeyValue;
use opentelemetry::propagation::Extractor;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::Sampler;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::TracerProviderBuilder;
use serde_json::Map;
use serde_json::Value;
use tracing::Dispatch;
use tracing::Event;
use tracing::Instrument as _;
use tracing::Subscriber;
use tracing::dispatcher::WeakDispatch;
use tracing::field::Field;
use tracing::field::Visit;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing::span::Record;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;

const SERVICE: &str = "jjforge-vcs";

/// The route of a request that no route matched.
const UNMATCHED: &str = "unmatched";

/// Installs the global subscriber and the W3C Trace Context propagator. The
/// provider it returns sends the remaining spans when it shuts down.
pub(crate) fn install(filter: &str, endpoint: Option<&Uri>) -> anyhow::Result<SdkTracerProvider> {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let provider = provider(endpoint)?;

    tracing_subscriber::registry()
        .with(EnvFilter::try_new(filter)?)
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer(SERVICE)))
        .with(JsonLines::new(std::io::stdout))
        .try_init()?;
    Ok(provider)
}

/// Gives each `sync/v1` request a server span that continues the caller's
/// `traceparent`, named after the route as the semantic conventions name
/// an HTTP server span, and writes the request's line when it ends.
pub(crate) async fn trace_http(
    route: Option<MatchedPath>,
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let parent = caller(request.headers());
    let method = request.method().to_string();
    // A path that no route matched may hold any name a caller sent, so the
    // span names none.
    let route = route.map_or_else(|| UNMATCHED.to_owned(), |route| route.as_str().to_owned());
    let span = tracing::info_span!(
        "HTTP request",
        otel.name = format!("{method} {route}"),
        otel.kind = "server",
        http.request.method = method,
        http.route = route,
        http.response.status_code = tracing::field::Empty,
        jjforge.org.id = tracing::field::Empty,
        jjforge.repo.id = tracing::field::Empty,
        jjforge.principal.id = tracing::field::Empty,
        jjforge.refusal = tracing::field::Empty,
    );
    span.set_parent(parent).ok();

    let start = Instant::now();
    let response = next.run(request).instrument(span.clone()).await;
    let status = response.status();
    span.record("http.response.status_code", status.as_u16());
    let outcome = if status.is_server_error() {
        Outcome::Failed
    } else if status.is_client_error() {
        Outcome::Rejected
    } else {
        Outcome::Ok
    };
    span.in_scope(|| {
        tracing::info!(
            method,
            route,
            status = status.as_u16(),
            outcome = outcome.name(),
            duration_ms = millis(start),
            "request"
        );
    });
    response
}

/// Gives each gRPC call a server span that continues the caller's
/// `traceparent`, as [`trace_http`] does for `sync/v1`, so the gate records
/// its context on the call's own span, and writes the call's line when it
/// ends.
#[derive(Clone)]
pub(crate) struct TraceGrpc;

impl<S> tower::Layer<S> for TraceGrpc {
    type Service = TracedGrpc<S>;

    fn layer(&self, inner: S) -> TracedGrpc<S> {
        TracedGrpc(inner)
    }
}

#[derive(Clone)]
pub(crate) struct TracedGrpc<S>(S);

impl<S, ReqBody, ResBody> tower::Service<http::Request<ReqBody>> for TracedGrpc<S>
where
    S: tower::Service<http::Request<ReqBody>, Response = http::Response<ResBody>>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, request: http::Request<ReqBody>) -> Self::Future {
        let route = request.uri().path().trim_start_matches('/').to_owned();
        let (service, method) = route.split_once('/').unwrap_or((&route, ""));
        let span = tracing::info_span!(
            "gRPC request",
            otel.name = route,
            otel.kind = "server",
            rpc.system = "grpc",
            rpc.service = service,
            rpc.method = method,
            rpc.grpc.status_code = tracing::field::Empty,
            jjforge.org.id = tracing::field::Empty,
            jjforge.repo.id = tracing::field::Empty,
            jjforge.principal.id = tracing::field::Empty,
            jjforge.refusal = tracing::field::Empty,
        );
        span.set_parent(caller(request.headers())).ok();
        let start = Instant::now();
        let response = span.in_scope(|| self.0.call(request));

        Box::pin(async move {
            let response = response.instrument(span.clone()).await;
            // tonic answers a failed call with its status in the headers, and
            // a successful one with OK in the trailers.
            let status = response.as_ref().map_or(GRPC_UNKNOWN, |response| {
                response
                    .headers()
                    .get("grpc-status")
                    .and_then(|status| status.to_str().ok()?.parse().ok())
                    .unwrap_or(GRPC_OK)
            });
            span.record("rpc.grpc.status_code", status);
            let outcome = match status {
                GRPC_OK => Outcome::Ok,
                GRPC_UNKNOWN
                | GRPC_DEADLINE_EXCEEDED
                | GRPC_UNIMPLEMENTED
                | GRPC_INTERNAL
                | GRPC_UNAVAILABLE
                | GRPC_DATA_LOSS => Outcome::Failed,
                _ => Outcome::Rejected,
            };
            span.in_scope(|| {
                tracing::info!(
                    route,
                    status,
                    outcome = outcome.name(),
                    duration_ms = millis(start),
                    "request"
                );
            });
            response
        })
    }
}

// The gRPC status codes that mean vcs failed, not the caller.
const GRPC_OK: u16 = 0;
const GRPC_UNKNOWN: u16 = 2;
const GRPC_DEADLINE_EXCEEDED: u16 = 4;
const GRPC_UNIMPLEMENTED: u16 = 12;
const GRPC_INTERNAL: u16 = 13;
const GRPC_UNAVAILABLE: u16 = 14;
const GRPC_DATA_LOSS: u16 = 15;

/// How a request ended, on its line.
enum Outcome {
    Ok,
    /// The caller's mistake or a refusal, whose reason the line names.
    Rejected,
    /// A failure of vcs.
    Failed,
}

impl Outcome {
    fn name(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }
}

fn millis(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The caller's trace from its `traceparent`.
fn caller(headers: &http::HeaderMap) -> opentelemetry::Context {
    struct Headers<'a>(&'a http::HeaderMap);
    impl Extractor for Headers<'_> {
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).and_then(|value| value.to_str().ok())
        }
        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(http::HeaderName::as_str).collect()
        }
    }

    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&Headers(headers))
    })
}

/// The provider's settings, which the tests share. It records every span,
/// whatever the sampled flag of a caller's `traceparent` says, and continues
/// the caller's trace ID, so no caller can turn off the record of its
/// requests and the ID that `jf` prints finds them, as ADR 0005 decides.
fn builder() -> TracerProviderBuilder {
    let resource = Resource::builder()
        .with_service_name(SERVICE)
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build();
    SdkTracerProvider::builder()
        .with_resource(resource)
        .with_sampler(Sampler::AlwaysOn)
}

/// A provider without an endpoint still gives each span a trace, so log lines
/// and calls onward carry it.
fn provider(endpoint: Option<&Uri>) -> anyhow::Result<SdkTracerProvider> {
    let mut provider = builder();
    if let Some(endpoint) = endpoint {
        // The exporter's reqwest shares the workspace's TLS features, which
        // leave the crypto provider to the binary, as in jf. A second
        // install, from a test, fails and changes nothing.
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let exporter = SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint.to_string())
            .build()?;
        provider = provider.with_batch_exporter(exporter);
    }
    Ok(provider.build())
}

/// Writes each event to `W` as one JSON object, with the text in `message`
/// and the trace that wrote it in `trace_id` and `span_id`, the names the log
/// collector and `VictoriaLogs` read. A layer, not a formatter, because only a
/// layer can reach the span's OpenTelemetry context while `tracing` handles the
/// event.
pub(crate) struct JsonLines<W> {
    writer: W,
    dispatch: OnceLock<WeakDispatch>,
}

impl<W> JsonLines<W> {
    pub(crate) fn new(writer: W) -> Self {
        Self {
            writer,
            dispatch: OnceLock::new(),
        }
    }
}

impl<S, W> Layer<S> for JsonLines<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: layer::Context<'_, S>) {
        let mut fields = RequestContext::default();
        attrs.record(&mut fields);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(fields);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: layer::Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(fields) = span.extensions_mut().get_mut::<RequestContext>()
        {
            values.record(fields);
        }
    }

    fn on_register_dispatch(&self, dispatch: &Dispatch) {
        let _ = self.dispatch.set(dispatch.downgrade());
    }

    fn on_event(&self, event: &Event<'_>, ctx: layer::Context<'_, S>) {
        let mut timestamp = String::new();
        let _ = SystemTime.format_time(&mut Writer::new(&mut timestamp));

        let mut line = Fields(Map::new());
        line.insert("timestamp", timestamp);
        line.insert("level", event.metadata().level().as_str());
        line.insert("target", event.metadata().target());
        event.record(&mut line);
        // Each line names its request's context, so a search by
        // organization, repository, or principal finds every line it wrote.
        for span in ctx
            .event_scope(event)
            .into_iter()
            .flat_map(tracing_subscriber::registry::Scope::from_root)
        {
            if let Some(fields) = span.extensions().get::<RequestContext>() {
                line.0.extend(fields.0.clone());
            }
        }

        let dispatch = self.dispatch.get().and_then(WeakDispatch::upgrade);
        let context = ctx
            .event_span(event)
            .zip(dispatch)
            .and_then(|(span, dispatch)| {
                tracing_opentelemetry::get_otel_context(&span.id(), &dispatch)
            });
        if let Some(context) = context {
            let span = context.span().span_context().clone();
            if span.is_valid() {
                line.insert("trace_id", span.trace_id().to_string());
                line.insert("span_id", span.span_id().to_string());
            }
        }

        let _ = writeln!(self.writer.make_writer(), "{}", Value::Object(line.0));
    }
}

/// The fields of a request's span that each line inside it repeats.
const CONTEXT: [&str; 4] = [
    "jjforge.org.id",
    "jjforge.repo.id",
    "jjforge.principal.id",
    "jjforge.refusal",
];

/// The [`CONTEXT`] fields that a span has recorded.
#[derive(Default)]
struct RequestContext(Map<String, Value>);

impl Visit for RequestContext {
    fn record_str(&mut self, field: &Field, value: &str) {
        if CONTEXT.contains(&field.name()) {
            self.0.insert(field.name().to_owned(), value.into());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if CONTEXT.contains(&field.name()) {
            self.0
                .insert(field.name().to_owned(), format!("{value:?}").into());
        }
    }
}

/// An event's fields, each under its own name.
struct Fields(Map<String, Value>);

impl Fields {
    fn insert(&mut self, name: &str, value: impl Into<Value>) {
        self.0.insert(name.to_owned(), value.into());
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field.name(), value);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field.name(), value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field.name(), value);
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field.name(), value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.insert(field.name(), format!("{value:?}"));
    }
}

/// Runs `future` under a subscriber that keeps the spans it finishes, so a
/// test can read what a request recorded. It fails the test when an event
/// fires outside a span, because every request must be traceable, as ADR
/// 0005 decides.
#[cfg(test)]
pub(crate) async fn finished_spans<F: std::future::Future>(
    future: F,
) -> (F::Output, Vec<opentelemetry_sdk::trace::SpanData>) {
    use tracing::instrument::WithSubscriber as _;

    // A request continues its caller's trace only through the propagator
    // that `install` sets.
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let exporter = opentelemetry_sdk::trace::InMemorySpanExporter::default();
    let provider = builder().with_simple_exporter(exporter.clone()).build();
    let orphans = Orphans::default();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")))
        .with(orphans.clone());
    let output = future.with_subscriber(subscriber).await;
    #[allow(clippy::unwrap_used, reason = "only tests call it")]
    let spans = exporter.get_finished_spans().unwrap();
    #[allow(clippy::unwrap_used, reason = "only tests call it")]
    let orphans = orphans.0.lock().unwrap();
    assert!(orphans.is_empty(), "events outside a span: {orphans:?}");
    (output, spans)
}

/// The targets of the events that fired outside any span.
#[cfg(test)]
#[derive(Clone, Default)]
struct Orphans(std::sync::Arc<std::sync::Mutex<Vec<String>>>);

#[cfg(test)]
impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Orphans {
    fn on_event(&self, event: &Event<'_>, ctx: layer::Context<'_, S>) {
        if ctx.event_span(event).is_none()
            && let Ok(mut orphans) = self.0.lock()
        {
            orphans.push(event.metadata().target().to_owned());
        }
    }
}

/// The names of the events on `spans`.
#[cfg(test)]
pub(crate) fn events(spans: &[opentelemetry_sdk::trace::SpanData]) -> Vec<String> {
    spans
        .iter()
        .flat_map(|span| span.events.iter())
        .map(|event| event.name.to_string())
        .collect()
}

/// The value of `key` on any of `spans`.
#[cfg(test)]
pub(crate) fn attribute(spans: &[opentelemetry_sdk::trace::SpanData], key: &str) -> Option<String> {
    spans
        .iter()
        .flat_map(|span| &span.attributes)
        .find(|attribute| attribute.key.as_str() == key)
        .map(|attribute| attribute.value.to_string())
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;
    use std::sync::Mutex;

    use opentelemetry::trace::SpanContext;
    use opentelemetry::trace::SpanId;
    use opentelemetry::trace::TraceFlags;
    use opentelemetry::trace::TraceId;
    use opentelemetry::trace::TraceState;

    use tracing_opentelemetry::OpenTelemetrySpanExt;

    use super::*;

    const TRACE: &str = "4bf92f3577b34da6a3ce929d0e0e4736";

    /// Collects what the formatter writes.
    #[derive(Clone, Default)]
    struct Output(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_call_continues_the_callers_trace() {
        use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
        use opentelemetry_sdk::trace::InMemorySpanExporter;
        use tower::Layer;
        use tower::Service;

        use crate::grpc::Echo;

        const CALLER: &str = "00f067aa0ba902b7";

        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
        let spans = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(spans.clone())
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut service = TraceGrpc.layer(EchoServiceServer::new(Echo));

        tracing::subscriber::with_default(subscriber, || {
            runtime.block_on(async {
                let request = http::Request::post("/echo.v1.EchoService/Echo")
                    .header("content-type", "application/grpc")
                    .header("traceparent", format!("00-{TRACE}-{CALLER}-01"))
                    .body(tonic::body::Body::empty())
                    .unwrap();
                service.call(request).await.unwrap();
            });
        });

        let spans = spans.get_finished_spans().unwrap();
        let call = spans
            .iter()
            .find(|span| span.name.contains("EchoService/Echo"))
            .unwrap();
        assert_eq!(call.span_context.trace_id().to_string(), TRACE);
        assert_eq!(call.parent_span_id.to_string(), CALLER);
    }

    #[tokio::test]
    async fn a_request_records_its_route_and_status_on_its_span() {
        use axum::http::StatusCode;
        use tower::ServiceExt as _;

        let app = axum::Router::new()
            .route(
                "/repos/{id}",
                axum::routing::get(|| async { StatusCode::NOT_FOUND }),
            )
            .layer(axum::middleware::from_fn(trace_http));
        let request = http::Request::get("/repos/7")
            .body(axum::body::Body::empty())
            .unwrap();

        let (_, spans) = finished_spans(app.oneshot(request)).await;

        assert_eq!(spans[0].name, "GET /repos/{id}");
        assert_eq!(
            attribute(&spans, "http.response.status_code").as_deref(),
            Some("404")
        );
    }

    #[test]
    fn a_provider_with_an_endpoint_builds() {
        let endpoint = Uri::from_static("http://localhost:4318/v1/traces");

        provider(Some(&endpoint)).unwrap().shutdown().unwrap();
    }

    #[test]
    fn a_line_names_its_trace_and_span() {
        let provider = provider(None).unwrap();
        let output = Output::default();
        let writer = output.clone();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer(SERVICE)))
            .with(JsonLines::new(move || writer.clone()));
        let caller = SpanContext::new(
            TraceId::from_hex(TRACE).unwrap(),
            SpanId::from_hex("00f067aa0ba902b7").unwrap(),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        );

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("call");
            span.set_parent(opentelemetry::Context::new().with_remote_span_context(caller))
                .unwrap();
            span.in_scope(|| tracing::info!(repo = "forge", "probe"));
        });

        let line: Value = serde_json::from_slice(&output.0.lock().unwrap()).unwrap();
        assert_eq!(line["message"], "probe");
        assert_eq!(line["repo"], "forge");
        assert_eq!(line["level"], "INFO");
        assert_eq!(line["trace_id"], TRACE);
        assert_ne!(line["span_id"], "00f067aa0ba902b7");
    }

    /// Runs `future` under a subscriber that writes JSON lines, and returns
    /// what it returned with the lines it wrote.
    async fn lines<F: std::future::Future>(future: F) -> (F::Output, Vec<Value>) {
        use tracing::instrument::WithSubscriber as _;

        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
        let provider = builder().build();
        let output = Output::default();
        let writer = output.clone();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer(SERVICE)))
            .with(JsonLines::new(move || writer.clone()));
        let returned = future.with_subscriber(subscriber).await;
        let lines = String::from_utf8(output.0.lock().unwrap().clone())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        (returned, lines)
    }

    #[tokio::test]
    async fn a_caller_can_not_turn_off_its_trace() {
        use tower::ServiceExt as _;

        let app = axum::Router::new()
            .route("/", axum::routing::get(|| async { "hi" }))
            .layer(axum::middleware::from_fn(trace_http));
        let request = http::Request::get("/")
            .header("traceparent", format!("00-{TRACE}-00f067aa0ba902b7-00"))
            .body(axum::body::Body::empty())
            .unwrap();

        let (_, spans) = finished_spans(app.oneshot(request)).await;

        let span = spans.first().expect("the request has a span");
        assert_eq!(span.span_context.trace_id().to_string(), TRACE);
    }

    #[tokio::test]
    async fn a_request_writes_one_line_when_it_ends() {
        use axum::http::StatusCode;
        use tower::ServiceExt as _;

        let app = axum::Router::new()
            .route(
                "/repos/{id}",
                axum::routing::get(|| async { StatusCode::NOT_FOUND }),
            )
            .layer(axum::middleware::from_fn(trace_http));
        let request = http::Request::get("/repos/7")
            .body(axum::body::Body::empty())
            .unwrap();

        let (_, lines) = lines(app.oneshot(request)).await;

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["message"], "request");
        assert_eq!(lines[0]["method"], "GET");
        assert_eq!(lines[0]["route"], "/repos/{id}");
        assert_eq!(lines[0]["status"], 404);
        assert_eq!(lines[0]["outcome"], "rejected");
        assert!(lines[0]["duration_ms"].is_number());
        assert!(lines[0]["trace_id"].is_string());
    }

    #[tokio::test]
    async fn a_grpc_call_writes_one_line_when_it_ends() {
        use jjforge_proto::echo::v1::echo_service_server::EchoServiceServer;
        use tower::Layer as _;
        use tower::Service as _;

        use crate::grpc::Echo;

        let mut service = TraceGrpc.layer(EchoServiceServer::new(Echo));
        let request = http::Request::post("/echo.v1.EchoService/Echo")
            .header("content-type", "application/grpc")
            .body(tonic::body::Body::empty())
            .unwrap();

        let (_, lines) = lines(async { service.call(request).await }).await;

        let line = lines
            .iter()
            .find(|line| line["message"] == "request")
            .unwrap();
        assert_eq!(line["route"], "echo.v1.EchoService/Echo");
        assert!(line["status"].is_number());
        assert!(line["outcome"].is_string());
    }

    #[test]
    fn a_line_inside_a_request_carries_its_context() {
        let output = Output::default();
        let writer = output.clone();
        let subscriber =
            tracing_subscriber::registry().with(JsonLines::new(move || writer.clone()));

        tracing::subscriber::with_default(subscriber, || {
            let request = tracing::info_span!(
                "request",
                jjforge.org.id = tracing::field::Empty,
                jjforge.principal.id = "ada",
            );
            request.record("jjforge.org.id", "acme");
            request.in_scope(|| {
                tracing::info_span!("step").in_scope(|| tracing::info!("probe"));
            });
        });

        let line: Value = serde_json::from_slice(&output.0.lock().unwrap()).unwrap();
        assert_eq!(line["jjforge.org.id"], "acme");
        assert_eq!(line["jjforge.principal.id"], "ada");
    }

    #[tokio::test]
    async fn no_span_or_line_names_a_path_no_route_matched() {
        use tower::ServiceExt as _;

        let app = axum::Router::new()
            .route("/", axum::routing::get(|| async { "hi" }))
            .layer(axum::middleware::from_fn(trace_http));
        let request = http::Request::get("/sync/v1/ada-lovelace/private-notes/nowhere")
            .body(axum::body::Body::empty())
            .unwrap();

        let (_, spans) = finished_spans(app.clone().oneshot(request)).await;
        let request = http::Request::get("/sync/v1/ada-lovelace/private-notes/nowhere")
            .body(axum::body::Body::empty())
            .unwrap();
        let (_, lines) = lines(app.oneshot(request)).await;

        let recorded = format!("{spans:?} {lines:?}");
        assert!(!recorded.contains("ada-lovelace"), "{recorded}");
        assert_eq!(lines[0]["route"], UNMATCHED);
    }
}

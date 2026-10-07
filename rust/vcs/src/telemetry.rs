//! Logs and traces, as ADR 0005 decides. A log line is one JSON object on
//! stdout with the trace that wrote it. Spans go to an OTLP endpoint when one
//! is set, and a caller's `traceparent` continues either way.

use std::fmt;
use std::io::Write;
use std::sync::OnceLock;

use http::Uri;
use opentelemetry::KeyValue;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use serde_json::Map;
use serde_json::Value;
use tracing::Dispatch;
use tracing::Event;
use tracing::Subscriber;
use tracing::dispatcher::WeakDispatch;
use tracing::field::Field;
use tracing::field::Visit;
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

/// A provider without an endpoint still gives each span a trace, so log lines
/// and calls onward carry it.
fn provider(endpoint: Option<&Uri>) -> anyhow::Result<SdkTracerProvider> {
    let resource = Resource::builder()
        .with_service_name(SERVICE)
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build();
    let mut provider = SdkTracerProvider::builder().with_resource(resource);
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
        use tonic_tracing_opentelemetry::middleware::server::OtelGrpcLayer;
        use tower::Layer;
        use tower::Service;

        use crate::echo::Echo;

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
        let mut service = OtelGrpcLayer::default().layer(EchoServiceServer::new(Echo));

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
}

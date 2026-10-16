# 0005. Logs, traces, and metrics through OpenTelemetry

Status: proposed, 2026-10-07. Deciders: jjforge maintainers.

## Context

A request crosses `jf` or the browser, the server, vcs, Postgres, and the
object store. When it fails or is slow, the people who run jjforge must find
where, starting from a trace ID that a person quotes. Today the server
exposes Prometheus metrics through Micrometer and vcs through the `metrics`
crate. vcs logs with `println!`, and no binary records a trace.

The infra repository runs VictoriaMetrics with 90 days of metrics,
VictoriaLogs with 30 days of logs collected from container stdout, and
Grafana. It runs no trace store.

- A `VMPodScrape` reads both binaries through the chart's ports named
  `management` and `metrics`. The alert `JjforgeServerErrors` reads
  Micrometer's `http_server_requests_seconds_count`, and `TargetDown` fires
  when a binary stops answering the scrape.
- An app's namespace sends nothing outside itself except DNS queries and
  calls to the API server. The observability namespace admits only itself,
  Traefik, and the API server. A scrape reaches jjforge, and nothing jjforge
  sends reaches a store.

The measurement store in the
[architecture](../architecture.md#building-blocks) measures the software that
people build on jjforge. This record covers telemetry about jjforge itself.

| Option                                                                    | Cost                                                  |
| ------------------------------------------------------------------------- | ----------------------------------------------------- |
| Each language's own tools, Prometheus, and no traces                      | No request can be followed from one binary to another |
| A vendor's agent and backend                                              | Lock-in, and a bill per host                          |
| OpenTelemetry, with OTLP, W3C Trace Context, and the semantic conventions | The Rust trace SDK is beta and still breaks. Chosen   |

| Option for the server                                                                      | Cost                                                                                  |
| ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| The OpenTelemetry Java agent                                                               | A second instrumentation next to Micrometer, matched by version to Spring, and no AOT |
| `spring-boot-starter-opentelemetry`, which bridges Micrometer Observation to OpenTelemetry | Chosen                                                                                |

| Option for vcs and `jf`                                                                 | Cost                                                                                       |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| The OpenTelemetry API for spans and `tracing` for events, as opentelemetry-rust advises | tonic emits `tracing` spans, which the export would miss                                   |
| `tracing`, exported through `tracing-opentelemetry`                                     | A bridge outside the OpenTelemetry project, which calls it a workaround at an edge. Chosen |

| Option for logs                                  | Cost                                                                                           |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| OTLP from each binary                            | The server needs a third-party Logback bridge, and a crash loses the lines still in the buffer |
| JSON lines on stdout, which the cluster collects | Each binary writes the trace ID into the line itself. Chosen                                   |

| Option for metrics  | Cost                                                                       |
| ------------------- | -------------------------------------------------------------------------- |
| OTLP, pushed        | A network path out of each namespace, new alerts, and no `up` series       |
| Prometheus, scraped | A second transport next to OTLP. Chosen, because the cluster scrapes today |

| Option for the trace store | Cost                                                                          |
| -------------------------- | ----------------------------------------------------------------------------- |
| Grafana Tempo              | A second vendor's stack next to VictoriaMetrics and VictoriaLogs              |
| VictoriaTraces             | Before 1.0, its storage format may change. Chosen, because traces live 7 days |

## Decision

### Signals

| Signal | Answers                                                    | Transport            | Store           | Retention                   |
| ------ | ---------------------------------------------------------- | -------------------- | --------------- | --------------------------- |
| Trace  | Where one request spent its time, and where it failed      | OTLP                 | VictoriaTraces  | 7 days                      |
| Metric | How many, how fast, and how full, over time                | Prometheus, scraped  | VictoriaMetrics | 90 days                     |
| Log    | What a process did that its spans and metrics don't record | JSON lines on stdout | VictoriaLogs    | 30 days                     |
| Audit  | Who did what                                               | A row and its event  | Postgres        | As long as its organization |

- Telemetry is expendable. A fact that must last, such as who changed what,
  is a row in Postgres, as [ADR 0002](0002-state-boundaries-and-tokens.md)
  decides, and never only a log line.
- The infra repository runs the stores, sets their retention, and opens the
  network path to them. The chart installs no store.
- `shared/deploy/compose.yaml` runs VictoriaTraces as the twin of the trace
  store.

### Traces

```mermaid
flowchart LR
    jf -- "traceparent" --> server
    web["web app"] --> server
    jf -- "traceparent, sync/v1" --> vcs
    server -- "traceparent, source/v1" --> vcs
    server --> pg[("Postgres")]
    vcs --> store[("object store")]
    server -. OTLP .-> collector["OpenTelemetry Collector"]
    vcs -. OTLP .-> collector
    collector --> traces[("VictoriaTraces")]
```

- The chart passes `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` to both binaries. A
  binary without it exports nothing and runs as before.
- The endpoint is an OpenTelemetry Collector in the infra repository, which
  every app's namespace reaches. It only receives, because the trace store
  serves reads and deletes on the port that takes writes.
- Each binary sets `service.name` to `jjforge-server` or `jjforge-vcs`, and
  `service.version` to its release. The chart sets
  `deployment.environment.name` through `OTEL_RESOURCE_ATTRIBUTES`.
- Every binary propagates W3C Trace Context and nothing else. Baggage stays
  off, because every binary would forward what a caller sets.
- Each request a binary serves gets a server span. Each call that leaves the
  process gets a client span, whether HTTP, gRPC, SQL, or the object store.
  Spans follow the OpenTelemetry semantic conventions.
- Tracing is cross-cutting, so no endpoint can forget it. The libraries and
  one place per binary trace every request, and the checks below refuse
  tracing code anywhere else.
- The server uses `spring-boot-starter-opentelemetry`, and Spring gRPC traces
  its calls to vcs.
- vcs records spans with `tracing` and exports them through
  `tracing-opentelemetry` and `opentelemetry-otlp`.
  `tonic-tracing-opentelemetry` in `main` continues a trace at every gRPC
  service.
- `jf` exports nothing, because it runs on people's machines. It starts a
  trace per command, and the one client it builds sends `traceparent` with
  every request, so the server and vcs continue the trace of `jf`.
- The web app sends no trace context while the OpenTelemetry browser
  instrumentation is experimental. The server starts the trace.
- The server names the trace of every response in a `traceresponse` header,
  from W3C Trace Context Level 2, whatever answers it. `jf` prints its trace
  ID on failure and the web app shows the header's, so a person can quote it.
- Every binary samples by its parent, and a new trace by the ratio of its
  trace ID, set to 1.0 until the trace store's volume asks for less. When
  it drops, the Collector samples by the whole trace and keeps every error.
- Every request is traceable. Each gRPC method and HTTP route that a binary
  serves has a span under the caller's trace. The span is the request's
  record, one wide event: who in `jjforge.principal.id`, where in
  `jjforge.org.id` and `jjforge.repo.id`, what it did, and how it ended. It
  names who also when the request is refused, so a trace shows who tried.
  A call that names no repository, such as `echo/v1`, names what it did.
- Work outside a request, such as a scheduled job or an event listener,
  runs in a span of its own, so these rules hold for it too.
- In vcs, the `server` side of the store crate adds spans under the
  request's: one per `Repo` call, one per step of a publish, and a client
  span per object store call. It is the one library that vcs adds spans to,
  so one trace shows a request of `jf` down to the object store. Each span
  names what it did:

| Span                       | Attributes                                                                                                                                 |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `repo head`                | `jjforge.operation.id`                                                                                                                     |
| `repo operations`          | `jjforge.operation.since`, `jjforge.operations.count`                                                                                      |
| `repo missing`             | `jjforge.ids.asked`, `jjforge.ids.missing`                                                                                                 |
| `repo fetch`               | `jjforge.ids.asked`, `jjforge.fetch.bytes`                                                                                                 |
| `repo upload`              | `jjforge.upload.objects`, `jjforge.upload.bytes`, `jjforge.segment.id`                                                                     |
| `repo publish`             | `jjforge.operation.expected`, `jjforge.publish.bookmarks`, `jjforge.publish.segments`, `jjforge.publish.outcome`, `jjforge.operation.id`   |
| `echo`                     | `jjforge.echo.bytes`, never the message                                                                                                    |
| `advance`                  | `jjforge.operations.read`                                                                                                                  |
| `object_store {operation}` | `jjforge.object_store.operation`, `jjforge.object_store.key`, and for a write `jjforge.object_store.bytes` and `jjforge.object_store.swap` |

- The outcome of a publish is `moved`, `unchanged`, or `stale`, and
  `jjforge.operation.id` is where `HEAD` is after it. A count never names
  what it counts, such as a bookmark, because no signal holds repository
  content.

### Metrics

- The chart keeps the ports named `management` and `metrics`, which the
  infra scrape selects.
- A metric that a library defines keeps the library's name, because the
  infra alerts read it.
- A metric of jjforge's own takes its name from the semantic conventions
  where they define one, and starts with `jjforge.` otherwise, such as
  `jjforge.push.conflicts`.
- A label takes its values from a set that the code bounds, such as a method
  or a status code. An ID never labels a metric. It goes on the span or the
  log line.

### Logs

- Each binary writes one JSON object per line to stdout, with the text in
  `message`, which the collector reads, and `trace_id` and `span_id` while a
  span is active. The server uses the structured logging of Spring Boot, and
  vcs uses `tracing-subscriber`.
- A message is a constant, and what varies goes in fields, so a search for a
  message finds every line that wrote it.
- A line is an event, never narration. Inside a request, a binary writes a
  line only when a step changed the request's course, such as a retry, a
  lost race, a cache that starts over, or a reload. No line repeats what the
  request's span records, such as one line per request, and none narrates,
  such as one on entering a function.
- A line inside a request falls inside its span, so it carries `trace_id`
  and `span_id`, and trace views show it on the span. OpenTelemetry
  deprecates the API for span events in favor of such log records.
- A client error writes no line, because the caller made the mistake. A
  failure of the binary itself writes one at ERROR, so it outlasts a trace
  that is sampled out or lost.
- Outside any span, only the start and the stop of a process, and what keeps
  it from running, write a line.
- No signal holds a secret, a token, a credential header, a request body,
  repository content, or personal data. A principal appears as its ID.

| Level | When                                                                                           |
| ----- | ---------------------------------------------------------------------------------------------- |
| ERROR | A person must act, such as after a failure of the binary or lost work                          |
| WARN  | The binary handled something unexpected, such as a repository created again under the same IDs |
| INFO  | A request changed course as expected, such as a lost race, or the process starts or stops      |
| DEBUG | A diagnosis needs detail. Off where jjforge is deployed, and turned on per logger              |

vcs writes these events inside a request:

| Event                 | Span                | When                                                                  |
| --------------------- | ------------------- | --------------------------------------------------------------------- |
| `key sets reload`     | the request's       | A token names a key that vcs doesn't know, so it fetches the key sets |
| `catalog starts over` | `advance`           | A repository was created again under the same IDs, as WARN            |
| `swap lost`           | `object_store swap` | Another writer moved `HEAD` first                                     |

### Checks

| Rule                                                                                                                                    | Check                                                                                                                    |
| --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| The server logs through SLF4J, never through `System.out` or `java.util.logging`                                                        | `ArchitectureTest`, ArchUnit                                                                                             |
| vcs logs through `tracing`, never through `println!` or `eprintln!`                                                                     | clippy `print_stdout` and `print_stderr`                                                                                 |
| Only the server's `telemetry` module uses a tracing API                                                                                 | `ArchitectureTest`, ArchUnit                                                                                             |
| Every response of the server names its trace                                                                                            | `TraceResponseTest`                                                                                                      |
| Only `forge::Client` builds an HTTP client in `jf`                                                                                      | clippy `disallowed-methods`                                                                                              |
| A request that carries `traceparent` has spans of the server and vcs under its trace                                                    | `mise run e2e`, which queries the twin                                                                                   |
| Every gRPC method and `sync/v1` route of vcs has a span under the caller's trace that names its organization, repository, and principal | `every_call_is_traceable_with_its_org_repo_and_principal` and `every_route_is_traceable_with_its_org_repo_and_principal` |
| An event of vcs in a request falls inside a span                                                                                        | `finished_spans`, under which every vcs test that reads spans runs, fails a test on an event outside one                 |
| Each event of vcs falls inside its request's span                                                                                       | a vcs or store test per event                                                                                            |
| A failure of vcs writes an ERROR line                                                                                                   | `a_server_error_that_axum_answers_writes_a_line`                                                                         |

## Consequences

- While the ratio is 1.0, traces are the only record of each request. If it
  drops and a question needs every request, each server span also writes
  one INFO line from its attributes, a canonical log line, and this record
  changes.
- An `@ApplicationModuleListener` runs after its request, on another
  thread. Its span continues the request's trace only once the server
  propagates the context, and an event published again after a restart
  starts a trace of its own. A test holds this once a module publishes its
  first event.
- The server's spans name no organization, repository, or principal until
  it authenticates, as #6 plans, and resolves the names in its paths to IDs.
- Traces wait for the infra repository. It runs VictoriaTraces and the
  Collector, lets every app's namespace reach the Collector, adds the trace
  store to Grafana, and links `trace_id` in VictoriaLogs to it.
- An upgrade of VictoriaTraces before 1.0 may discard the stored traces,
  which costs at most 7 days of them.
- A caller outside the cluster decides whether its trace is kept. That holds
  while the ratio is 1.0. Lowering it changes this record.
- A SQL client span needs an instrumented data source, chosen with the first
  module that uses Postgres.
- Once the trace SDK of opentelemetry-rust is stable, vcs moves its edges to
  that API and this record changes.
- No check holds the label rule or the rule against secrets yet.

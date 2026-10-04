# 0005. vcs is the storage kernel, and the server owns the product

Status: accepted, 2026-10-03. Deciders: jjforge maintainers.

## Context

jjforge is also a training program. Apprentices learn Spring Boot by building
the forge's features, and one maintainer keeps the Rust code. With the object
store as the only source of truth, as
[ADR 0006](0006-object-store-is-the-source-of-truth.md) decides, the server
needs somewhere to keep its own state, and something must stop concurrent
writers from losing each other's work.

The building blocks grow at different speeds, and apprentices work on them in
parallel. Microservices from the start would add deployment and network
failure modes before any module needs them. A monolith without boundaries
grows into one tangle.

The options were:

- Each Spring module owns Postgres tables. Familiar, but Postgres becomes a
  second source of truth.
- The server writes its own streams to the object store with the S3 SDK. One
  hop less, but the log format gets a second implementation, and apprentices
  write storage code.
- vcs offers a small, generic contract for streams, names, and leases, and the
  server keeps its state through it. Rust holds the storage mechanisms, and
  Spring holds every feature.

## Decision

- vcs holds VCS semantics and storage mechanisms, and nothing a person would
  call a feature. The server holds every feature: organizations, identity,
  policy, review, landing, hooks, workflows, and notifications.
- vcs serves `kernel/v1`, described in [proto/kernel/v1](../../proto/kernel/v1/):
  1. Streams: append with an expected version, read from a version, and
     subscribe to a prefix from a cursor. A subscription also carries every
     repository's operations.
  2. Names: claim a globally unique name.
  3. Leases: acquire and renew a named lease with an epoch. A write that
     carries a stale epoch is refused.
- Events in a stream are opaque bytes to vcs. The server owns their schemas.
- The tenant is the unit of ordering. Every append to a tenant's streams, and
  every publish to its repositories, goes through one owner at a time, chosen
  on a hash ring, which batches them into one compare-and-swap on the tenant's
  log. A stream is a view of that log, and a cursor is a position in it. A
  subscription to a prefix that spans tenants subscribes to each.
- The server is one Spring Boot app, structured with Spring Modulith. Each
  module is a top-level package.
- Each module owns a set of stream prefixes and one read-model schema in
  Postgres. A module reads only its own streams and tables.
- A module writes by event sourcing: it loads an aggregate's stream, checks its
  rules, and appends with the version it read. On a conflict it reloads and
  tries again.
- A projector writes a read model and its cursor in one transaction, so it
  applies each event once.
- Modules talk through events. Other processes subscribe to the streams. No
  separate event bus runs.
- The Spring Modulith verification runs as a test and fails the build on a
  dependency that crosses a module boundary.

## Consequences

- A rule that spans aggregates, such as a unique name, needs its own stream or
  a claimed name. There are no transactions across streams.
- A command returns the version it wrote. A query that must see that write
  waits until its read model reaches the version, or reads the stream.
- A module can move into its own service later by taking its streams and its
  read-model schema with it.

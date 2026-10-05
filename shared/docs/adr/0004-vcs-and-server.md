# 0004. vcs owns repositories, and the server owns the product

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

jjforge is also a training program. Apprentices learn Spring Boot by building
the forge's features, and one maintainer keeps the Rust code. The building
blocks grow at different speeds, and apprentices work on them in parallel.
Microservices from the start would add deployment and network failure modes
before any module needs them. A monolith without boundaries grows into one
tangle.

jjforge must run on one machine for evaluation and in a cluster, without two
code paths. Loki, Mimir, and Tempo run one binary that serves any subset of
its roles. That pays off once a role needs to scale on its own, and costs a
flag, a profile, and a chart mode per binary before that.

The options were:

- vcs offers a generic storage kernel of streams, names, and leases, and the
  server keeps its state through it. One source of truth, but every feature
  pays the event-sourcing cost, and the Rust maintainer owns a database.
- The server writes its own streams to the object store with the S3 SDK.
  The log format gets a second implementation, and apprentices write storage
  code.
- vcs holds only VCS semantics, and each Spring module owns Postgres tables.

## Decision

- vcs holds VCS semantics and storage mechanisms, and nothing a person would
  call a feature: the jj backend and the sync protocol of
  [ADR 0002](0002-native-jj-without-git.md), and `source/v1` for reading
  repositories from inside the cluster.
- The server holds every feature: organizations, identity, policy, review,
  landing, hooks, workflows, and notifications, with its state in Postgres,
  as [ADR 0003](0003-where-state-lives.md) decides.
- The server is one Spring Boot app, structured with Spring Modulith. Each
  module is a top-level package and owns one Postgres schema. A module reads
  only its own tables. `ModulesTest` runs the Modulith verification and fails
  the build on a dependency that crosses a module boundary.
- Modules talk through events. A module publishes a Spring Modulith event in
  the transaction that changes its rows, and the event publication registry
  delivers it after the commit and retries it after a crash. No separate
  event bus runs.
- The server learns about pushes from vcs through a server-streaming call in
  `source/v1`. It keeps the head it last saw per repository in Postgres and
  reads the operation log from there, so a dropped stream loses nothing.
- Each binary runs every role it has. There is no role flag and no Spring
  profile for roles. The chart runs one Deployment of each binary and scales
  each by replica count. A split by role waits for a measurement that asks
  for it.
- The object store is addressed by a URL. `s3://` is used wherever jjforge
  runs with its twins, and `file://` runs vcs on its own, without containers.
- The infra repository runs SeaweedFS and Postgres in the cluster. The chart
  takes only their connection configuration and installs neither. For
  development and evaluation, `shared/deploy/compose.yaml` runs both
  binaries with the twins of
  [ADR 0009](0009-specified-tested-built.md).
- No cache service until a requirement needs one, measured. Every replica
  caches immutable objects in memory and on local disk.

## Consequences

- A rule that spans modules goes through events, and holds a moment after
  the commit, not in it.
- A module can move into its own service later by taking its schema and its
  events with it.

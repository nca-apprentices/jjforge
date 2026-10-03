# 0008. Postgres holds only read models

Status: accepted, 2026-10-03. Deciders: jjforge maintainers.

## Context

The object store is the only source of truth, as
[ADR 0006](0006-object-store-is-the-source-of-truth.md) decides, but it can't
answer a list, a filter, or a join. The server's modules need those, and
apprentices learn Spring Data, JPA, Flyway, and SQL by building them.

The options were:

- Postgres as a second source of truth. Familiar, but it needs backups and
  point-in-time recovery, and one primary caps writes.
- Distributed SQL such as CockroachDB or YugabyteDB. Scales writes, but costs
  latency, money, and operational knowledge the team doesn't have yet.
- Embedded stores on the object store, such as an LSM tree per partition. No
  database to run, but nothing apprentices already know.
- Postgres holding only read models. Events from the streams build them, and
  replaying the streams rebuilds them.

## Decision

- jjforge uses Postgres for read models only, run in the cluster by the
  CloudNativePG operator.
- Each server module owns one schema, as
  [ADR 0005](0005-storage-kernel.md) decides, and writes it only from events.
- A projector stores its cursor in the same transaction as its read model.
- Postgres needs no backups. A module rebuilds its schema by dropping it and
  replaying from the start.
- The tenant leads every key, as
  [ADR 0003](0003-tenant-keys-and-pagination.md) decides.

## Consequences

- Losing Postgres costs only the time to replay, so a single instance is
  enough for small installations. A large one adds a replica to cut that time.
- A read model may lag its stream. A query that must see a write waits for the
  version the command returned.
- A query that nobody planned needs a new read model.
- When one Postgres no longer keeps up, a module's read model moves to an
  instance of its own. Nothing needs to migrate, because it can be rebuilt.

# 0008. Postgres, not distributed SQL

Status: superseded by [ADR 0018](0018-object-store-is-the-source-of-truth.md), 2026-10-03. Deciders: jjforge maintainers.

## Context

jjforge needs transactions, a schema per module, and a database people already
know. Distributed SQL scales writes across nodes, but costs latency, money, and
operational knowledge that the team doesn't have yet.

Options: Postgres run by the CloudNativePG (CNPG) operator, a managed cloud
database, or distributed SQL such as CockroachDB or YugabyteDB.

## Decision

- jjforge uses Postgres, run in the cluster by CNPG.
- The schema stays portable to distributed SQL: the tenant leads every key and
  a transaction stays in one tenant, as
  [ADR 0003](0003-tenant-keys-and-pagination.md) decides.

## Consequences

This decision is revisited when one of these holds:

- One organization's write load no longer fits a single primary.
- Read replicas no longer keep up with reads.
- An installation needs active writes in more than one region.

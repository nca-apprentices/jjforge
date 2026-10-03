# 0018. The object store is the only source of truth

Status: accepted, 2026-10-03. Deciders: jjforge maintainers. Supersedes
[ADR 0006](0006-object-storage.md) and [ADR 0008](0008-postgres-on-cnpg.md).

## Context

ADR 0006 kept the objects in SeaweedFS and the operation heads in Postgres, so
a head could move in a transaction. Every write then depends on two stateful
systems, and Postgres caps write scaling at one primary.

jj already avoids locks: operations are immutable objects, and only the head
moves. Object stores now offer conditional writes. `If-None-Match: *` creates
an object only if it doesn't exist, and `If-Match` replaces it only if it
hasn't changed since it was read. SeaweedFS evaluates both atomically across
its cluster, as do S3, GCS, and R2. One compare-and-swap on a small object is
the only coordination a repository needs.

The options were:

- Objects in the object store and heads in Postgres, as ADR 0006 decided.
- Heads in a consensus group, such as embedded Raft. Fast, but every node
  becomes stateful and Raft becomes the team's to run.
- Everything in the object store, with heads moved by compare-and-swap. Every
  compute node is stateless, but each write costs a round trip to the store,
  and queries need indexes built from the data.

## Decision

- The object store holds every piece of state that can't be rebuilt.
  Everything else is derived from it.
- Storage is laid out by tenant, as [ADR 0003](0003-tenant-keys-and-pagination.md)
  requires:

  ```text
  t/{org}/pool/seg/{segment}   packed objects, shared by the organization's repositories
  t/{org}/pool/idx/{segment}   hash to segment and offset, read with range requests
  t/{org}/r/{repo}/op/{hash}   operations and views
  t/{org}/r/{repo}/HEAD        the current operation
  t/{org}/s/{stream}/...       domain streams, in the same log format
  names/{name}                 globally unique names
  ```

- Objects are packed into immutable segments with an index. A single object is
  never one stored object, because each request costs time and money.
- `HEAD` moves only with `If-Match`. A writer that loses the race reloads,
  merges its operation onto the new head, and tries again.
- A globally unique name is claimed with `If-None-Match: *`.
- Unreachable objects are collected by a background job that holds a lease, as
  [ADR 0020](0020-storage-kernel.md) describes.
- Postgres holds only read models that can be rebuilt by replaying the streams.
  It needs no backups. A module drops its schema and replays to rebuild it.
- Single-repository reads come from the log, so a read after a write sees the
  write. Lists and searches come from read models, which may lag.

## Consequences

- A write costs one conditional request to the object store, 20 to 200 ms on
  most stores. Writers batch, as [ADR 0020](0020-storage-kernel.md) decides.
- Losing Postgres costs only the time to replay. Losing the object store loses
  data, so its replication and backups are the ones that matter.
- A query that nobody planned needs a new read model.

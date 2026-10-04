# 0006. The object store is the only source of truth

Status: accepted, 2026-10-03. Deciders: jjforge maintainers.

## Context

A repository holds many immutable objects and one small pointer that changes
often: the operation head. Local disks tie a repository to one node and make
scaling out a migration. Keeping the heads in a database, so a head moves in a
transaction, makes every write depend on two stateful systems, and the
database caps write scaling at one primary.

jj already avoids locks: operations are immutable objects, and only the head
moves. Object stores now offer conditional writes. `If-None-Match: *` creates
an object only if it doesn't exist, and `If-Match` replaces it only if it
hasn't changed since it was read. SeaweedFS evaluates both atomically across
its cluster since 4.29, as does S3. GCS offers the same through generation
numbers, and R2 accepts the headers. One compare-and-swap on a small object
is the only coordination a repository needs.

The options were:

- Local disks per vcs node, or a shared file system.
- Objects in the object store and heads in a database.
- Heads in a consensus group, such as embedded Raft. Fast, but every node
  becomes stateful and Raft becomes the team's to run.
- Everything in the object store, with heads moved by compare-and-swap. Every
  compute node is stateless, but each write costs a round trip to the store,
  and queries need indexes built from the data.

## Decision

- The object store holds every piece of state that can't be rebuilt.
  Everything else is derived from it, as
  [ADR 0008](0008-postgres-holds-read-models.md) decides for the read models.
- vcs is stateless. Any replica serves any repository.
- Storage is laid out by tenant, as
  [ADR 0003](0003-tenant-keys-and-pagination.md) requires:

  ```text
  t/{org}/pool/seg/{segment}   packed objects, shared by the organization's repositories
  t/{org}/pool/idx/{segment}   hash to segment and offset, read with range requests
  t/{org}/r/{repo}/op/{hash}   operations and views
  t/{org}/r/{repo}/HEAD        the current operation
  t/{org}/log/...              the tenant's log of stream events, see ADR 0005
  names/{name}                 globally unique names
  ```

- Objects are packed into immutable segments with an index. A single object is
  never one stored object, because each request costs time and money. A
  writer, vcs or `jf`, buffers the objects of one operation and writes them
  as one segment before it writes the operation.
- An operation names the segments that hold the objects it made visible,
  so a reader finds any object from the log alone: it walks the segments,
  newest first, and caches each index.
- `HEAD` moves only with `If-Match` naming the entity tag the writer read. A
  writer that loses the race reloads, merges its operation onto the new head,
  and tries again.
- A globally unique name is claimed with `If-None-Match: *`.
- The bucket has no versioning, and no write uses any other condition.
  SeaweedFS evaluates only these two atomically, and only on a bucket without
  versioning.
- Unreachable objects are collected by a background job that holds a lease, as
  [ADR 0005](0005-storage-kernel.md) describes.
- Single-repository reads come from the log, so a read after a write sees the
  write.

## Consequences

- A write costs one conditional request to the object store, 20 to 200 ms on
  most stores. Writers batch, as [ADR 0005](0005-storage-kernel.md) decides.
- Losing the object store loses data, so its replication and backups are the
  ones that matter.

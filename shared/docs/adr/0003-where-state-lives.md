# 0003. Repositories live in the object store, and the server's state in Postgres

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

A repository holds many immutable objects and one small pointer that changes
often: the operation head. Local disks tie a repository to one node, and
heads in a database make every push depend on two stateful systems. jj
already avoids locks: operations are immutable, and only the head moves.
Object stores now offer conditional writes. `If-None-Match: *` creates an
object only if it doesn't exist, and `If-Match` replaces it only if it hasn't
changed since it was read. SeaweedFS evaluates both atomically since 4.29, as
do S3, GCS, and R2.

The server's modules need lists, filters, joins, unique names, and a read
that sees the write before it. Apprentices learn Spring Data, JPA, Flyway,
and SQL by building them. jjforge must also run on one machine and later
shard by organization without a rewrite.

The options were:

- Repositories on local disks, on a shared file system, with heads in a
  database, in a consensus group such as embedded Raft, or whole in the
  object store with heads moved by compare-and-swap. Only the last keeps
  every vcs node stateless, at a round trip per write.
- One object pool per organization, which shares objects between its
  repositories but leaves a deleted repository's objects to a collector, or
  one pool per repository, which duplicates objects between forks. No
  requirement names a fork yet.
- The server's state in Postgres, or in event streams on the object store
  with Postgres as a read model that can be rebuilt. The second needs no
  backups, but a read lags its write, a unique name needs a claim, and
  apprentices learn event sourcing before Spring.
- IDs from sequences, UUIDv4, or UUIDv7, and lists by offset or by cursor.
  Offset pagination breaks when rows arrive between pages, and random UUIDs
  scatter index writes.

## Decision

- The object store holds every repository: its objects, its operations, and
  its head. vcs is stateless, and any replica serves any repository. Each
  repository has its own pool:

  ```text
  t/{org}/r/{repo}/pool/seg/{segment}   packed objects
  t/{org}/r/{repo}/pool/idx/{segment}   hash to segment and offset, read with range requests
  t/{org}/r/{repo}/op/{hash}            operations and views
  t/{org}/r/{repo}/HEAD                 the current operation
  ```

- Objects are packed into immutable segments with an index, because each
  request costs time and money. A writer writes the objects of one operation
  as one segment before the operation, and the operation names its segments,
  so a reader finds any object from the log alone.
- `HEAD` is created with `If-None-Match: *` and moves only with `If-Match`
  naming the entity tag the writer read. A writer that loses the race
  reloads, merges its operation onto the new head, and tries again. The
  bucket has no versioning, and no write uses any other condition, because
  SeaweedFS evaluates only these two atomically.
- Nothing is collected. jj keeps every operation, and a deleted repository is
  removed by its prefix. A repository exists in the store before the server
  records it, so a `HEAD` the server never recorded is unreachable and costs
  storage only.
- Postgres holds every piece of the server's state: organizations,
  repositories, principals, sessions, policy decisions, and whatever later
  modules add. Each server module owns one schema and migrates it with
  Flyway. A command changes rows and publishes its events in one transaction,
  so the next read sees the write.
- A rule within one module is a constraint, such as a unique index on
  `(org_id, name)`. Work that needs a single owner takes a Postgres advisory
  lock.
- The CloudNativePG operator runs Postgres in the cluster and backs it up to
  the object store, with point-in-time recovery.
- The organization leads every key and every index. IDs are UUIDv7. A
  transaction stays within one tenant. Every list answers one page and an
  opaque `next` cursor while more pages exist, and no list answers an offset
  or a total count.

## Consequences

- A push costs one conditional request to the object store, 20 to 200 ms on
  most stores, and never touches Postgres.
- Losing either store loses data, so both have backups and a restore drill.
- A migration is permanent. A wrong one is fixed by another migration.
- Writes stop at one primary. This project stays far below that ceiling, and
  a module's schema can move to an instance of its own when it gets there.
- A query that spans organizations reads from events or a projection, not
  from one transaction.
- A fork, when a requirement names one, either copies the objects or
  references the segments of its parent. Both change this record.

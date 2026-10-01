# 0003. The tenant leads every key

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

jjforge must run on one machine and later scale out without a rewrite. Data
sharded by organization needs every key to name its organization. Offset
pagination breaks when rows arrive between pages, and random UUIDs scatter
index writes.

Options for IDs: database sequences, UUIDv4, or UUIDv7. For lists: offset
pagination or cursors.

## Decision

- The organization (the tenant) leads every primary key and every index.
- IDs are UUIDv7: unique without coordination, and ordered by time.
- A transaction stays within one tenant.
- Every list answers one page and an opaque `next` cursor while more pages
  exist. No list answers an offset or a total count.

## Consequences

A query that spans organizations, such as an administrator's report, reads
from events or a projection, not from one transaction.

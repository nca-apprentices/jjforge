# 0019. The public protocols are REST and the sync protocol

Status: accepted, 2026-10-03. Deciders: jjforge maintainers. Supersedes
[ADR 0002](0002-public-api-is-http.md).

## Context

ADR 0002 made REST the one public API and git smart HTTP the one other public
protocol. [ADR 0017](0017-native-jj-without-git.md) removes git, so `jf` needs
a way to move objects and operations.

Moving objects differs from every other operation. Objects are immutable and
named by their hash, so any cache may keep them forever. Uploads are large, and
the head moves only when everything it references has arrived.

The options were:

- Add the sync operations to the REST contract. The server would have to serve
  them, and every byte would pass through it.
- A gRPC service for sync. Streaming is natural, but HTTP caches can't cache
  gRPC, and ADR 0002 already kept gRPC away from clients.
- A small HTTP protocol of its own, served by vcsd, with Protocol Buffers
  bodies. Immutable reads are plain `GET` requests that a CDN can serve.

## Decision

- The public API is HTTP only, and it has two parts:
  1. REST, plus server-sent events for live updates, served by the server and
     described in [openapi.yaml](../../openapi.yaml). The CLI, the web app, and
     agents use the same API, and there are no private endpoints.
  2. The sync protocol `sync/v1`, served by vcsd under `/sync/v1/` on the same
     host. Its messages are in [proto/sync/v1](../../proto/sync/v1/), and its
     routes are listed there.
- `sync/v1` addresses a repository by its organization and repository IDs,
  which the REST answer for a repository returns as its clone address. A
  rename doesn't break a clone.
- Objects are read with `GET`, and their answers are immutable. vcsd checks the
  token before it serves or redirects to a short-lived signed URL.
- vcsd verifies the hash of every uploaded object before it stores it.
- A publish names the head it expects. vcsd moves the head only when every
  referenced object is present, the token's scope covers each changed bookmark,
  a protected bookmark moves only under the landing token, and the change
  ownership policy holds. vcsd decides all of this offline, from the token.
- Both parts answer errors as `application/problem+json` with the same problem
  codes, so the CLI maps them to the same exit codes.
- gRPC runs only inside the cluster, under `vcsd/v1` and `kernel/v1`.

## Consequences

`sync/v1` isn't in TypeSpec, because the server doesn't serve it and the
generated interfaces would require a controller for each operation. `buf
breaking` guards it as it guards the internal contracts.

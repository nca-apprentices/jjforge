# 0002. The public API is HTTP only

Status: superseded by [ADR 0019](0019-public-protocols.md), 2026-10-03. Deciders: jjforge maintainers.

## Context

The echo skeleton lets the CLI call vcsd over gRPC directly, and the web app
call the server over REST. Two public contracts mean two authentication paths,
two compatibility promises, and a CLI that can do what the web app can't.

Options: gRPC for every client, with gRPC-Web in the browser; REST for every
client; or both. Browsers can't speak native gRPC, and gRPC-Web needs a proxy.
Agents and scripts reach REST with any HTTP client.

## Decision

- The public API is HTTP only. It is REST, plus server-sent events (SSE) for
  live updates, and [openapi.yaml](../../openapi.yaml) describes all of it.
- The CLI, the web app, and agents use the same API. There are no private
  endpoints.
- gRPC runs only between the server and vcsd, under `shared/proto/vcsd/v1/`.
- Git smart HTTP, which vcsd serves for the git bridge, is the one other public
  protocol. Git defines it, not jjforge. See
  [ADR 0007](0007-git-bridge-first.md).

## Consequences

`shared/proto/` becomes internal. `buf breaking` still runs, because the server
and vcsd run different versions during a rolling deploy. The CLI's direct
`echo/v1` call goes away with the echo skeleton.

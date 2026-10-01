# Architecture

jjforge starts as a forge for Jujutsu repositories and grows into a platform
where people and agents build software with scoped, recorded access. The
decisions behind this page are in [adr/](adr/README.md).

## Components

```text
jf (CLI) ──┐
           ├─ REST + SSE ─► server ── gRPC vcsd/v1 ──► vcsd ──► SeaweedFS
web app ───┘                 │  │                      ▲  │
                             │  └──► Redpanda          │  │
                             └─────► Postgres ◄────────┼──┘
git client ──────────── git smart HTTP ────────────────┘
```

- The CLI, the web app, and agents use one public API of REST and server-sent
  events, described in [openapi.yaml](../openapi.yaml). There are no private
  endpoints. See [ADR 0002](adr/0002-public-api-is-http.md).
- gRPC runs only between the server and vcsd, described in
  [proto/vcsd/v1](../proto/vcsd/v1/).
- vcsd is stateless. It stores git-format objects in SeaweedFS and keeps the
  operation heads in Postgres. See [ADR 0006](adr/0006-object-storage.md).
- vcsd serves git smart HTTP itself, so a git client pushes and clones without
  the server in the path. See [ADR 0007](adr/0007-git-bridge-first.md).
- The server is a Spring Modulith app. Modules talk through events,
  which also go to Redpanda. See [ADR 0005](adr/0005-modular-monolith.md).

Today only the echo skeleton runs: `jf echo`, the web form, and
`POST /api/echo` each pass a message through vcsd over `echo/v1`. Every other
operation in the contract answers 501.

| Path                         | What it is                                                              |
| ---------------------------- | ----------------------------------------------------------------------- |
| `shared/openapi.yaml`        | The public REST contract                                                |
| `shared/proto/echo/v1/`      | The echo contract, removed once a real operation runs                   |
| `shared/proto/vcsd/v1/`      | The internal contract between the server and vcsd                       |
| `shared/http/`               | Hurl requests against the REST contract                                 |
| `shared/docs/`               | These documents                                                         |
| `shared/site/`               | The docs site, built from `shared/docs/`                                |
| `shared/deploy/chart/`       | The Helm chart, released with every tag                                 |
| `shared/deploy/compose.yaml` | The evaluation tier: everything on one machine                          |
| `shared/config/`             | Tool configuration and the mise tasks                                   |
| `rust/cli/`                  | The `jf` command                                                        |
| `rust/vcsd/`                 | The data core                                                           |
| `rust/proto/`                | The Rust stubs for `shared/proto/`                                      |
| `server/`                    | Kotlin Spring Boot. `identity/` and `source/` hold the stub controllers |
| `web/`                       | A pnpm workspace: `apps/`, `features/`, and `shared/`                   |

## Building blocks

| #   | Block                          | What it holds                                                                   | State       |
| --- | ------------------------------ | ------------------------------------------------------------------------------- | ----------- |
| 1   | Identity and authorization     | Principals, sign-in, tokens, policy, secrets, and the audit log                 | Planned     |
| 2   | Source plane                   | Repositories, changes, the operation log, conflicts, stacks, and the git bridge | Planned     |
| 3   | Spec and intent plane          | Specs linked to change IDs and traced to measurements                           | Planned     |
| 4   | Execution plane                | Sandboxes, durable workflows, quotas, and the build cache                       | Planned     |
| 5   | Component registry             | Manifests, install as grant, and supply chain checks                            | Planned     |
| 6   | Event bus and hooks            | Typed events, blocking gates, and observers                                     | Planned     |
| 7   | Measurement store              | Metrics, traces, and evaluations, keyed by change ID                            | Planned     |
| 8   | Artifact and attestation store | Content-addressed outputs and the attestations attached to them                 | Planned     |
| 9   | Control plane                  | Organizations, configuration from the forge, tenancy, and metering              | Planned     |
| 10  | Interfaces                     | The `jf` CLI, the web app, and the public API                                   | In progress |

The echo skeleton, one message through every component, is built.

## Invariants

1. Every object is addressable by content hash or change ID.
2. Every action is attributable to a principal with a delegation chain.
3. Every capability is an attenuated, expiring, identity-bound token.
4. Every state transition emits an event and is recorded.
5. Every component, including the platform's own, uses the same registry,
   permission, and sandbox path.

Every key starts with the tenant, so data can be sharded by organization later.
See [ADR 0003](adr/0003-tenant-keys-and-pagination.md).

## Dependency order

Identity and token broker → forge with an operation log event stream → event
bus and hook contract → sandbox runtime → component manifest and registry →
measurement store → spec plane → experimentation and promotion.

Specs and measurements depend on stable change IDs and events. The registry
depends on the permission model. Building the registry before the token model
would force a rewrite of every component. The
[roadmap](roadmap.md) follows this order.

## Releases and deployment

This repository is the product. The shared cluster in
[nca-apprentices/infra](https://github.com/nca-apprentices/infra) is one
installation of it. The boundary follows these rules:

- The infra repository consumes released artifacts only: the chart from
  `oci://ghcr.io/nca-apprentices/charts/jjforge` and the images from
  `ghcr.io/nca-apprentices`. It never references a path in this repository.
- A release doesn't deploy. Deploying is a change to `targetRevision` in the
  infra repository's `clusters/<env>/apps/jjforge/application.yaml`.
- The infra repository runs Postgres, Redpanda, and SeaweedFS, the backing
  services. The chart takes only their connection configuration. See
  [ADR 0010](adr/0010-backing-services-in-infra.md).

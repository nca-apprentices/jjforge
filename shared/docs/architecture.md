# Architecture

jjforge starts as a forge for Jujutsu repositories and grows into a platform
where people and agents build software with scoped, recorded access. The
decisions behind this page are in [adr/](adr/README.md).

## Components

```mermaid
flowchart TB
    subgraph clients[Clients]
        cli["jf (CLI)"]
        web[web app]
    end

    subgraph server["server: Spring Boot"]
        direction LR
        roles["roles: api, projector, worker"]
        domains["identity, policy, orgs, review, hooks"]
    end

    pg[("Postgres<br/>(read models)")]

    subgraph vcs["vcs: Rust"]
        vroles["roles: sync, source, writer, indexer"]
    end

    store[("object store<br/>the only source of truth")]

    cli -- "REST + SSE" --> server
    web -- "REST + SSE" --> server
    cli -- "sync/v1" --> vcs
    server -- "gRPC source/v1, kernel/v1" --> vcs
    server --> pg
    vcs --> store
```

| Path                         | What it is                                                              |
| ---------------------------- | ----------------------------------------------------------------------- |
| `shared/api/`                | The public REST contract in TypeSpec                                    |
| `shared/openapi.yaml`        | Compiled from `shared/api/` for the generators, ignored by git          |
| `shared/proto/echo/v1/`      | The echo contract, removed once a real operation runs                   |
| `shared/proto/source/v1/`    | The internal contract for reading repositories                          |
| `shared/proto/kernel/v1/`    | The internal contract for streams, names, and leases                    |
| `shared/proto/sync/v1/`      | The public sync protocol between `jf` and vcs                           |
| `shared/proto/store/v1/`     | The native object format                                                |
| `shared/proto/repos/v1/`     | The events of the server's repos module                                 |
| `shared/e2e/http/`           | Hurl scenarios against the REST contract                                |
| `shared/e2e/http/pending/`   | Scenarios whose operation still answers 501                             |
| `shared/e2e/cli/`            | Bats scenarios against the `jf` command                                 |
| `shared/docs/`               | These documents, with one design per epic in `design/`                  |
| `shared/site/`               | The docs site, built from `shared/docs/`                                |
| `shared/deploy/chart/`       | The Helm chart, released with every tag                                 |
| `shared/deploy/compose.yaml` | The evaluation tier: everything on one machine                          |
| `shared/config/`             | Tool configuration and the mise tasks                                   |
| `rust/cli/`                  | The `jf` command                                                        |
| `rust/vcs/`                  | The storage kernel                                                      |
| `rust/proto/`                | The Rust stubs for `shared/proto/`                                      |
| `jvm/`                       | Kotlin Spring Boot. `identity/` and `source/` hold the stub controllers |
| `web/`                       | A pnpm workspace: `apps/`, `features/`, and `shared/`                   |

## Building blocks

| #   | Block                          | What it holds                                                         |
| --- | ------------------------------ | --------------------------------------------------------------------- |
| 1   | Identity and authorization     | Principals, sign-in, tokens, policy, secrets, and the audit log       |
| 2   | Source plane                   | Repositories, changes, the operation log, conflicts, stacks, and sync |
| 3   | Spec and intent plane          | Specs linked to change IDs and traced to measurements                 |
| 4   | Execution plane                | Sandboxes, durable workflows, quotas, and the build cache             |
| 5   | Component registry             | Manifests, install as grant, and supply chain checks                  |
| 6   | Event bus and hooks            | Typed events, blocking gates, and observers                           |
| 7   | Measurement store              | Metrics, traces, and evaluations, keyed by change ID                  |
| 8   | Artifact and attestation store | Content-addressed outputs and the attestations attached to them       |
| 9   | Control plane                  | Organizations, configuration from the forge, tenancy, and metering    |
| 10  | Interfaces                     | The `jf` CLI, the web app, and the public API                         |

## Invariants

1. Every object is addressable by content hash or change ID.
2. Every action is attributable to a principal with a delegation chain.
3. Every capability is an attenuated, expiring, identity-bound token.
4. Every state transition emits an event and is recorded.
5. Every component, including the platform's own, uses the same registry,
   permission, and sandbox path.

## Dependency order

Identity and token broker → forge with an operation log event stream → event
bus and hook contract → sandbox runtime → component manifest and registry →
measurement store → spec plane → experimentation and promotion.

Specs and measurements depend on stable change IDs and events. The registry
depends on the permission model. Building the registry before the token model
would force a rewrite of every component. The initiatives follow this order.

## Releases and deployment

This repository is the product. The shared cluster in
[nca-apprentices/infra](https://github.com/nca-apprentices/infra) is one
installation of it. The boundary follows these rules:

- The infra repository consumes only what a
  [release](development.md#releasing) publishes. It never references a path in
  this repository.
- A release doesn't deploy. Deploying is a change to `targetRevision` in the
  infra repository's `clusters/<env>/apps/jjforge/application.yaml`.

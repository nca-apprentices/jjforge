# Architecture

## Components

```text
jjforge echo hi ──gRPC echo/v1──────────────────────────► vcsd
web form ─────────REST POST /api/echo──► server ──gRPC──► vcsd
```

| Path           | What it is                                                     |
| -------------- | -------------------------------------------------------------- |
| `proto/`       | The gRPC contract `echo/v1`                                    |
| `openapi.yaml` | The REST contract, implemented by the server                   |
| `rust/`        | The Cargo workspace                                            |
| `rust/cli/`    | The `jjforge` command                                          |
| `rust/vcsd/`   | The data core, serving `echo/v1`                               |
| `rust/proto/`  | The Rust stubs for `proto/`                                    |
| `server/`      | Kotlin Spring Boot: `POST /api/echo`, forwarded to vcsd        |
| `web/`         | React: a form that calls `/api/echo`, served by the server     |
| `chart/`       | The Helm chart, released with every tag                        |
| `compose.yaml` | The evaluation tier: server and vcsd on one machine            |

The ingress sends `/echo.v1.EchoService` to vcsd and everything else to the
server, so the CLI and the browser share one hostname.

## Releases and deployment

This repository is the product. The shared cluster in
[nca-apprentices/infra](https://github.com/nca-apprentices/infra) is one
installation of it. The boundary follows these rules:

- The infra repository consumes released artifacts only: the chart from
  `oci://ghcr.io/nca-apprentices/charts/jjforge` and the images from
  `ghcr.io/nca-apprentices`. It never references a path in this repository.
- A release doesn't deploy. Deploying is a change to `targetRevision` in the
  infra repository's `clusters/<env>/apps/jjforge/application.yaml`.

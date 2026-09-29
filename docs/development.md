# Development

Run these from the repository root, or from `rust/` for the Rust workspace.
Each has its own `mise.toml`: `mise install` provides its toolchain, and
`mise trust` once lets mise read it.

## Checking

```text
mise run fmt     # format every file, as .config/dprint.json configures
mise run lint    # formatting, prose, buf, helm, tsc, biome
mise run test    # gradle build, vitest, web build
```

In `rust/`, the same tasks cover the Rust code:

```text
mise run fmt     # cargo fmt
mise run lint    # cargo fmt --check, clippy
mise run test    # cargo test
```

CI runs lint and test in both places.

## Running

With podman, which builds both images from this directory:

```text
mise run up      # server on :8080, vcsd on :50052
mise run smoke   # in a second terminal: web, server, vcsd and cli
```

Set `JJFORGE_PORT` for both when something else holds port 8080.

Without containers:

```text
(cd rust && cargo run --bin vcsd)
gradle -p server bootRun
(cd rust && cargo run --bin jjforge -- echo hi)
curl -X POST localhost:8080/api/echo -H 'content-type: application/json' -d '{"message":"hi"}'
```

`JJFORGE_ENDPOINT` points the CLI at another vcsd, such as
`https://jjforge.example.com`.

## Trying the API

`http/` holds [Hurl](https://hurl.dev) files: requests with asserts on each
response. `mise run smoke` runs them against `mise run up`, and they run
against any other server too:

```text
hurl --test --variable server=https://jjforge.example.com http/*.hurl
```

Hurl cannot send gRPC, so vcsd is exercised through the `jjforge` CLI.

## Changing a contract

- `proto/`: the CLI is installed by people and updated when they feel like it,
  so the gRPC contract may only grow. CI runs `buf breaking` against `main`.
- `openapi.yaml`: written by hand. The server build generates Kotlin interfaces
  and models from it, and every controller implements one of those
  interfaces. `ControllerContractTest` fails on a controller that implements
  none or maps a route of its own.

## Releasing

A `v*` tag publishes the server and vcsd images, the CLI binaries, and the
chart to `oci://ghcr.io/nca-apprentices/charts/jjforge`, all with the same
version. Deploying it is a separate change in
[nca-apprentices/infra](https://github.com/nca-apprentices/infra).

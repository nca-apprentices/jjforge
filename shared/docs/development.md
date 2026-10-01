# Development

Run these from the repository root. `mise install` provides the whole
toolchain, and `mise trust` once lets mise read `mise.toml`. The tasks live in
`shared/config/mise/tasks/`, and `mise tasks` lists them.

## Checking

```text
mise run fmt     # format every file, as shared/config/dprint.json configures
mise run lint    # formatting, prose, links, buf, spec, workflows, Dockerfiles,
                 # task scripts, helm, tsc, biome
mise run test    # gradle build (tests, detekt, architecture, coverage), vitest,
                 # web build
```

The `rust:` tasks cover the Rust code:

```text
mise run rust:fmt     # cargo fmt
mise run rust:lint    # cargo fmt --check, clippy, rustdoc, machete, deny
mise run rust:test    # cargo test
```

CI runs lint, test, rust:lint, and rust:test.
[ADR 0015](adr/0015-checked-code-rules.md) lists the rules they enforce.
[ADR 0015](adr/0015-checked-code-rules.md) lists the rules they enforce.

## Running

With podman, which builds both images from the repository root:

```text
mise run up      # server on :8080, vcsd on :50052
mise run smoke   # in a second terminal: web, server, vcsd and cli
```

Set `JJFORGE_PORT` for both when something else holds port 8080.

Without containers:

```text
(cd rust && cargo run --bin vcsd)
gradle -p server bootRun
(cd rust && cargo run --bin jf -- echo hi)
curl -X POST localhost:8080/api/echo -H 'content-type: application/json' -d '{"message":"hi"}'
```

`JJFORGE_ENDPOINT` points the CLI at another vcsd, such as
`https://jjforge.example.com`.

## Trying the API

`shared/http/` holds [Hurl](https://hurl.dev) files: requests with asserts on
each response. `mise run smoke` runs them against `mise run up`, and they run
against any other server too:

```text
hurl --test --variable server=https://jjforge.example.com shared/http/*.hurl
```

Hurl cannot send gRPC, so vcsd is exercised through the `jf` CLI.

`mise run lint` also checks every relative link and anchor in the Markdown
with lychee. `mise run site:build` builds the docs site in `shared/site/` from
these documents.

## Changing a contract

A contract changes before its implementation, and every operation cites the
requirements it serves. [Planning](planning.md) has the rules.

- `shared/openapi.yaml`: the public API, written by hand. Every operation lists
  its requirement issues in `x-requirements`.
  - `mise run spec:lint` lints it with Redocly and is part of
    `mise run lint`.
  - `mise run spec:trace` checks that every operation cites only issues of
    type Requirement. The echo operation is exempt until it is removed.
  - `mise run spec:breaking` compares it with `main` using oasdiff. CI accepts
    a breaking change only when the PR title marks it with `!`, as in
    `feat!: rename the org field`.
  - The server build generates Kotlin interfaces and models from it, and one
    stub controller per tag implements them. An operation that isn't built
    yet answers 501. `ControllerContractTest` fails on a controller that
    implements no interface or maps a route of its own, and on an interface
    without a controller. See
    [ADR 0011](adr/0011-controllers-implement-contracts.md).
- `shared/proto/`: the internal contract the server uses to call vcsd. CI runs
  `buf breaking` against `main`, because the server and vcsd run different
  versions during a rolling deploy.

## Releasing

A `v*` tag publishes the server and vcsd images, the CLI binaries, and the
chart to `oci://ghcr.io/nca-apprentices/charts/jjforge`, all with the same
version. Deploying it is a separate change in
[nca-apprentices/infra](https://github.com/nca-apprentices/infra).

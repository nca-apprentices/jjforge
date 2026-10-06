# Development

Run these from the repository root. `mise install` provides the whole
toolchain, and `mise trust` once lets mise read `mise.toml`. The tasks live in
`shared/config/mise/tasks/`, and `mise tasks` lists them.

## Checking

```text
mise run fmt         # format every file, then cargo fmt
mise run lint        # every lint task below
mise run test        # every test task below
mise run site:build  # the docs site in shared/site/, from shared/docs/
```

Each top-level directory has its own tasks:

```text
mise run shared:lint  # formatting, prose, links and anchors, buf, spec,
                      # workflows, Dockerfiles, task scripts, helm
mise run jvm:lint     # detekt
mise run jvm:test     # gradle build (tests, detekt, architecture, coverage)
mise run web:lint     # tsc, biome
mise run web:test     # vitest, web build
mise run rust:fmt     # cargo fmt
mise run rust:lint    # cargo fmt --check, clippy, rustdoc, machete, deny
mise run rust:test    # cargo test
```

The REST contract has its own tasks:

```text
mise run api:build     # compile shared/api/ to shared/openapi.yaml
mise run api:lint      # TypeSpec formatting, then Redocly
mise run api:breaking  # compare the REST contract with main using oasdiff
```

CI runs each namespace as a job.
[ADR 0003](adr/0003-checked-code-rules.md) lists the rules they enforce.

## Running

With podman, which builds both images from the repository root:

```text
mise run up      # server on :8080, vcs and the twins of ADR 0004 behind it
mise run e2e     # in a second terminal: web, server, vcs and cli
mise run compose logs   # or any other docker-compose command, such as ps or down
```

Set `JJFORGE_PORT` for both when something else holds port 8080.

Without containers:

```text
mise run api:build
(cd rust && cargo run --bin vcs)
gradle -p jvm bootRun
(cd rust && cargo run --bin jf -- echo hi)
curl -X POST localhost:8080/api/v1/echo -H 'content-type: application/json' -d '{"message":"hi"}'
```

`JJFORGE_ENDPOINT` points the CLI at another forge, such as
`https://jjforge.example.com`.

## Trying the API

`shared/e2e/` holds the scenarios. [Hurl](https://hurl.dev) files under
`http/` assert on each response, and
[Bats](https://bats-core.readthedocs.io) files under `cli/` run the first
`jf` on the `PATH`. `mise run e2e` runs both against `mise run up`, and
they run against any other server too:

```text
hurl --test --variable server=https://jjforge.example.com shared/e2e/http/*.hurl
JJFORGE_ENDPOINT=https://jjforge.example.com bats shared/e2e/cli
```

## Releasing

A `v*` tag publishes, all with the same version, the server and vcs images to
`ghcr.io/nca-apprentices`, the CLI binaries, and the chart to
`oci://ghcr.io/nca-apprentices/charts/jjforge`.
[Architecture](architecture.md#releases-and-deployment) says how it is
deployed.

The release also writes `Formula/jf.rb` in
[nca-apprentices/homebrew-tap](https://github.com/nca-apprentices/homebrew-tap),
so `brew install nca-apprentices/tap/jf` installs the new `jf` on macOS
(Apple silicon) and Linux. `mise run release:formula <tag> <dir>` prints the
same formula from a directory of `jf-<target>` binaries. The tap's deploy key,
stored as the secret `HOMEBREW_TAP_DEPLOY_KEY`, lets the release push to it.

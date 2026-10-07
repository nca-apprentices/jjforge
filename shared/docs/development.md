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

`jf echo hi` prints the trace ID of its request on stderr, and so does the
echo page. The twin of the trace store shows that trace at
`http://localhost:10428/select/vmui`, from the server through vcs.
`JJFORGE_TRACES_PORT` moves the twin to another port.

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
they run against any other server too. A scenario with the line `# pending`
fails until its requirement is built:

```text
hurl --test --variable server=https://jjforge.example.com shared/e2e/http/*.hurl
JJFORGE_ENDPOINT=https://jjforge.example.com bats shared/e2e/cli
```

## Previews

A PR with the `preview` label runs on the cluster next to `jjforge-dev`, at
`https://jjforge-pr-<n>.nca-apprentices.dev`, behind the GitHub login of
nca-apprentices. The Preview workflow pushes the images of each new commit,
and Argo CD deploys them within two minutes. The PR shows the deployment, and
`success` means the head commit is live. A preview uses the database and
stores of `jjforge-dev`, and disappears when the PR merges, closes, or loses
the label. Fork PRs get none.

Its logs are in [VictoriaLogs](https://ops.nca-apprentices.dev/logs/select/vmui/)
under `kubernetes.pod_labels.app.kubernetes.io/instance` `jjforge-pr-<n>`, and
its traces in Grafana under `deployment.environment.name` `pr-<n>`.
The [operations guide](https://github.com/nca-apprentices/infra/blob/main/docs/operations.md#previews)
of infra says how the cluster runs it.

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

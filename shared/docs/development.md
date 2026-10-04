# Development

Run these from the repository root. `mise install` provides the whole
toolchain, and `mise trust` once lets mise read `mise.toml`. The tasks live in
`shared/config/mise/tasks/`, and `mise tasks` lists them.

## Checking

```text
mise run fmt     # format every file, then cargo fmt
mise run lint    # every lint task below
mise run test    # every test task below
```

Each top-level directory has its own tasks:

```text
mise run shared:lint  # formatting, prose, links, buf, spec, workflows,
                      # Dockerfiles, task scripts, helm
mise run jvm:lint     # detekt
mise run jvm:test     # gradle build (tests, detekt, architecture, coverage)
mise run web:lint     # tsc, biome
mise run web:test     # vitest, web build
mise run rust:fmt     # cargo fmt
mise run rust:lint    # cargo fmt --check, clippy, rustdoc, machete, deny
mise run rust:test    # cargo test
```

CI runs each namespace as a job.
[ADR 0015](adr/0015-checked-code-rules.md) lists the rules they enforce.

## Running

With podman, which builds both images from the repository root:

```text
mise run up      # server on :8080, vcs and the twins of ADR 0018 behind it
mise run e2e     # in a second terminal: web, server, vcs and cli
mise run compose logs   # or any other docker-compose command, such as ps or down
```

Set `JJFORGE_PORT` for both when something else holds port 8080.

Without containers:

```text
mise run spec:build
(cd rust && cargo run --bin vcs)
gradle -p jvm bootRun
(cd rust && cargo run --bin jf -- echo hi)
curl -X POST localhost:8080/api/v1/echo -H 'content-type: application/json' -d '{"message":"hi"}'
```

`JJFORGE_ENDPOINT` points the CLI at another forge, such as
`https://jjforge.example.com`.

## Trying the API

`shared/e2e/` holds the end-to-end scenarios. `shared/e2e/http/` holds
[Hurl](https://hurl.dev) files: requests with asserts on each response.
`shared/e2e/cli/` holds [Bats](https://bats-core.readthedocs.io) files, which
run the first `jf` on the `PATH`. `mise run e2e` runs both against
`mise run up`, and they run against any other server too.

```text
hurl --test --variable server=https://jjforge.example.com shared/e2e/http/*.hurl
JJFORGE_ENDPOINT=https://jjforge.example.com bats shared/e2e/cli
```

`mise run lint` also checks every relative link and anchor in the Markdown
with lychee. `mise run site:build` builds the docs site in `shared/site/` from
these documents.

## Changing a contract

The `jjforge-specify` skill in `.claude/skills/` walks through a spec PR, as
[ADR 0017](adr/0017-specified-before-built.md) decides, and the
`jjforge-spec-reviewer` agent in `.claude/agents/` reviews it.
`jjforge-implement` builds the requirement once the spec merges.
[AGENTS.md](../../AGENTS.md) routes each kind of work to its skill.

```text
mise run spec:build     # compile shared/api/ to shared/openapi.yaml
mise run spec:lint      # TypeSpec formatting, then Redocly
mise run spec:trace     # map each requirement to what cites it, and fail on a gap
mise run spec:breaking  # compare the REST contract with main using oasdiff
```

CI accepts a breaking REST change only when the PR title marks it with `!`, as
in `feat!: rename the org field`. `buf breaking` guards every proto against
`main`, because the server, vcs, and `jf` run different versions during a
rolling deploy and long after it.

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

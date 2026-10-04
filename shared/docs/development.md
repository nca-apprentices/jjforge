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

## Running

With podman, which builds both images from the repository root:

```text
mise run up      # server on :8080, vcsd behind it
mise run smoke   # in a second terminal: web, server, vcsd and cli
mise run compose logs   # or any other docker-compose command, such as ps or down
```

Set `JJFORGE_PORT` for both when something else holds port 8080.

Without containers:

```text
mise run spec:build
(cd rust && cargo run --bin vcsd)
gradle -p server bootRun
(cd rust && cargo run --bin jf -- echo hi)
curl -X POST localhost:8080/api/v1/echo -H 'content-type: application/json' -d '{"message":"hi"}'
```

`JJFORGE_ENDPOINT` points the CLI at another forge, such as
`https://jjforge.example.com`.

## Trying the API

`shared/http/` holds [Hurl](https://hurl.dev) files: requests with asserts on
each response. `mise run smoke` runs them against `mise run up`, and they run
against any other server too.

```text
hurl --test --variable server=https://jjforge.example.com shared/http/*.hurl
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
`main`, because the server, vcsd, and `jf` run different versions during a
rolling deploy and long after it.

## Releasing

A `v*` tag publishes, all with the same version, the server and vcsd images to
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

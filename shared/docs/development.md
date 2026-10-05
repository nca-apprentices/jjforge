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
mise run api:build
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

## Specifying and building

A requirement is specified in its epic's page under [specs/](specs/README.md)
before it is built, as [ADR 0017](adr/0017-specified-before-built.md)
decides. The Spec workflow fails a PR that closes a requirement no spec page
has a section for.

A spec PR, for one or more requirements of an epic:

1. Add each requirement's section to the epic's page, or create the page and
   its row in the specs README, in the format the README gives.
2. Write each scenario in `shared/e2e/http/pending/` or
   `shared/e2e/cli/pending/`. It uses concrete values and checks the problem
   code of every refusal. Link it from its section.
3. Change `shared/api/` and `shared/proto/` until every assert is
   expressible.
4. Run `mise run lint` and `mise run api:breaking`.
5. Title it `spec: <what people can do>`, and write `Part of #<epic>` in the
   body.

A build PR, for one requirement, by someone other than the spec's author:

1. Move the scenario out of `pending/` and update its link. Start
   `mise run up`, and watch `mise run e2e` fail.
2. Build along the Flow section. A controller overrides the generated method,
   as [ADR 0011](adr/0011-controllers-implement-contracts.md) decides.
3. A scenario or a flow that can't be built as written goes back to a spec PR.
   This PR changes nothing else in the spec.
4. Run `mise run test`, `mise run lint`, and `mise run e2e`. Check that
   `mise run compose logs` shows no error and that the state the Persistence
   section names exists, as
   [ADR 0018](adr/0018-external-systems-run-as-twins.md) decides.
5. Title it `feat(<module>): <what a person can do>`, and write `Closes #<n>`
   and what step 4 showed in the body.

```text
mise run api:build     # compile shared/api/ to shared/openapi.yaml
mise run api:lint      # TypeSpec formatting, then Redocly
mise run api:breaking  # compare the REST contract with main using oasdiff
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

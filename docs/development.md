# Development

Run these from the repository root. `mise install` provides the whole
toolchain, and `mise trust` once lets mise read `mise.toml`. The tasks live in
`config/mise/tasks/`, and `mise tasks` lists them.

```text
mise run setup  # toolchain, dependencies, and the generated files the IDE needs
mise run clean  # delete build output, dependencies, generated files, and
                # agent scratch files, but not local state such as .idea/
```

## Checking

```text
mise run fmt         # format every file, then cargo fmt
mise run lint        # every lint task below
mise run test        # every test task below
mise run site:build  # the docs site in site/, from docs/
```

The repository and each component have their own tasks:

```text
mise run repo:lint    # formatting, prose, links and anchors, buf, spec,
                      # workflows, Dockerfiles, task scripts, helm
mise run server:lint  # detekt
mise run server:test  # gradle build (tests, detekt, architecture, coverage),
                      # with Postgres in a container
mise run web:lint     # tsc, biome
mise run web:test     # vitest, web build
mise run core:fmt     # cargo fmt
mise run core:lint    # cargo fmt --check, clippy, rustdoc, machete, deny
mise run core:test    # cargo test
```

The REST contract has its own tasks:

```text
mise run api:build     # bundle contracts/openapi/ into contracts/openapi.yaml
mise run api:lint      # Redocly, with the rules in config/redocly.yaml
mise run api:redocly   # the pinned Redocly CLI with this config, as in `api:redocly lint`
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

## Principals

vcs authenticates no one until sign-in exists, as
[#6](https://github.com/nca-apprentices/jjforge/issues/6) plans. A request
names its principal, and vcs grants the scopes that
`deploy/twins/principals` gives that name. `jf` sends the name in
`JJFORGE_TOKEN`:

```text
set -x JJFORGE_TOKEN acme_member
jf clone http://localhost:8081/sync/v1/0199c9a0-0000-7000-8000-00000000ac3e/<repo>
```

The people of the seed are `acme_owner`, `acme_member`, and `other_owner`.
The server calls vcs as `server`. A script such as a `grpcurl` call to
`RepoService` sends `authorization: Bearer acme_owner`. A deployed vcs has no
principals file, so it refuses every request.

`jf echo hi` prints the trace ID of its request on stderr, and so does the
echo page. The twin of the trace store shows that trace at
`http://localhost:10428/select/vmui`, from the server through vcs.
`JJFORGE_TRACES_PORT` moves the twin to another port.

Without building the images, each in a terminal of its own:

```text
mise run twins         # postgres on :5432, seaweedfs, oidc, and traces on :10428
mise run core:serve    # vcs on :50052 and sync/v1 on :8081, repositories in deploy/.dev/store
mise run server:serve  # server on :8080, actuator on :8081
mise run web:serve     # web app on :5173, /api proxied to :8080
mise run site:serve    # docs site on :8000
gradle -p platform/server bootTestRun  # or the server with Postgres in a container
(cd core && cargo run --bin jf -- echo hi)
curl -X POST localhost:8080/api/v1/echo -H 'content-type: application/json' -d '{"message":"hi"}'
```

`JJFORGE_POSTGRES_PORT` moves Postgres off 5432 for `twins` and `server:serve`.

`JJFORGE_ENDPOINT` points the CLI at another forge, such as
`https://jjforge.example.com`.

## Trying the API

`e2e/` holds the scenarios. [Hurl](https://hurl.dev) files under
`http/` assert on each response, and
[Bats](https://bats-core.readthedocs.io) files under `cli/` run the first
`jf` on the `PATH`. `mise run e2e` runs both against `mise run up`, and
they run against any other server too. A scenario with the line `# pending`
fails until its requirement is built:

```text
hurl --test --variable server=https://jjforge.example.com e2e/http/*.hurl
JJFORGE_ENDPOINT=https://jjforge.example.com bats e2e/cli
```

## Previews

A PR with the `preview` label runs on the cluster next to `jjforge-dev`, at
`https://jjforge-pr-<n>.nca-apprentices.dev`, behind the GitHub login of
nca-apprentices. The Preview workflow pushes the images of each new commit,
and Argo CD deploys them within two minutes. The PR shows the deployment, and
`success` means the head commit is live. A preview uses the stores of
`jjforge-dev` and a database of its own, `jjforge-pr-<n>-db`, so its
migrations never reach the database of `jjforge-dev`. It disappears, with its
database, when the PR merges, closes, or loses the label. Fork PRs get none.

Its logs are in [VictoriaLogs](https://logs.nca-apprentices.dev/select/vmui/)
under `kubernetes.pod_labels.app.kubernetes.io/instance` `jjforge-pr-<n>`, and
its traces in Grafana under `deployment.environment.name` `pr-<n>`.
The [operations guide](https://github.com/nca-apprentices/infra/blob/main/docs/operations.md#previews)
of infra says how the cluster runs it.

## Reviews

`/jf-review <n>` reviews a PR in a Claude Code session. It runs the stack
from the PR's branch, backs each finding with a Hurl or Bats script that
fails, answers the open review threads, and says what blocks the merge. The
`jf-review` agent runs the same skill in the background, so the stack's logs
stay out of the session.

Two workflows run the same skill:

- **Review** runs the skill on each PR that is ready for review, and again
  on a comment that starts with `@claude review`. The job starts the stack,
  and the Claude GitHub App posts the review. A PR from a fork gets a review
  of its diff only, and runs no code.
- **Claude** answers `@claude` from anyone with write access in an issue, a
  PR, or a review, such as `@claude fix the failing scenario`. It pushes to
  the PR's branch as the Claude GitHub App, so its commits run CI. It doesn't
  run on a PR from a fork.

The token from `claude setup-token` lives in the `claude-review`
environment, which only `main` may use. The Review workflow builds a PR in a
job without the token, and runs its code only in containers and in the
sandbox around Claude's commands. Neither workflow runs the code of a PR
from a fork.

## Releasing

`gh workflow run release.yml -f version=v1.2.3` releases the head of `main`.
The Release workflow tags it and publishes, all with the same version, the
server and vcs images to `ghcr.io/nca-apprentices`, the CLI binaries, and the
chart to `oci://ghcr.io/nca-apprentices/charts/jjforge`. The `release-tags`
rule set keeps a `v*` tag from moving or disappearing once it exists.
[Architecture](architecture.md#releases-and-deployment) says how it is
deployed.

The release also writes `Formula/jf.rb` in
[nca-apprentices/homebrew-tap](https://github.com/nca-apprentices/homebrew-tap),
so `brew install nca-apprentices/tap/jf` installs the new `jf` on macOS
(Apple silicon) and Linux. `mise run release:formula <tag> <dir>` prints the
same formula from a directory of `jf-<target>` binaries. The tap's deploy key,
stored as the secret `HOMEBREW_TAP_DEPLOY_KEY` in the `release` environment,
lets the release push to it. Only `main` may use that environment.

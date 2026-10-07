# 0004. A requirement is specified, tested against twins, then built

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

A requirement said what must be true, but nothing checked how: acceptance
criteria lived in issues, no test cited a requirement, and the write paths
had no design. Apprentices learn the way a feature is specified, so it must
be the same for every feature. jjforge also depends on systems it doesn't
deploy, so a requirement can pass its tests and still fail against the
running system, and `mise run up` can't sign anyone in without a provider.

| Option                                                  | Cost                                                       |
| ------------------------------------------------------- | ---------------------------------------------------------- |
| Spec Kit, OpenSpec, or OpenFastTrace                    | A second tracker next to the issues, and the two drift     |
| One Markdown page per epic, held by the checks that run | Chosen                                                     |
| A mock of each external system in each test             | Checks only what its author believed                       |
| Shared instances of the real systems                    | Every laptop and CI depends on the network and on accounts |
| A twin of each system, started with jjforge everywhere  | Memory and startup time. Chosen                            |

## Decision

Every fact has one home, and nothing is restated:

| Fact        | Home                                                            |
| ----------- | --------------------------------------------------------------- |
| Status      | The requirement issue, which holds a statement and nothing else |
| Description | The epic's spec page                                            |
| Acceptance  | The scenarios, in Hurl for REST and in Bats for `jf`            |
| Shape       | The contract in `shared/api/` and `shared/proto/`               |

- One spec PR specifies an epic with every requirement in its page under
  [specs/](../specs/README.md), at most one epic ahead of its build.
  `mise run prose` holds the page to the format the README gives.
- A scenario is named `<issue number>-<slug>` after its requirement, and
  `mise run prose` fails a section without a scenario or a scenario without
  a section. One that needs something the contract lacks changes the
  contract, never the scenario.
- A scenario holds the line `# pending` until its build PR, and
  `mise run e2e` fails a pending scenario that passes.
- One build PR per requirement, by someone other than the spec's author,
  removes the line and builds it. The Spec workflow fails it unless the base
  branch specifies the requirement and no scenario of it is still pending.
  [Workflow](../workflow.md#specifying-and-building) gives the steps.
- A requirement is built when its scenarios pass against the system built
  from the PR with its twins, and the stored state matches the Persistence
  section.

Every external system has a twin: a local instance that speaks the same
protocol, seeded with known data.

| External system         | Twin                                                                         |
| ----------------------- | ---------------------------------------------------------------------------- |
| Postgres                | Postgres                                                                     |
| S3 object store         | SeaweedFS                                                                    |
| OpenID Connect provider | `mock-oauth2-server` from NAV. Keycloak starts slowly, Dex serves one issuer |
| Trace store             | VictoriaTraces, which indexes a trace after 1 second instead of 20           |

- A twin is taken off the shelf, and written here only when nothing
  conforms. `shared/deploy/compose.yaml` runs every twin from the start,
  integration tests start the same images, and no integration test replaces
  an external system with a mock.
- The seed is the test data: the organizations `acme` and `other`, each with
  its own issuer, owners, and members, in `shared/deploy/twins/`. The Hurl
  fixtures name the same people.
- The Architecture section of a spec page names the external systems its
  paths call, and a new one brings its twin in the spec PR.

## Consequences

A requirement has three states, each visible in the tree: unspecified,
specified with a pending scenario, and built. The checks keep the tree true.
A requirement that no request or `jf` command can observe says so in its
section and names the test that checks it. `mise run up` needs more memory and starts slower with every
twin, and a behavior in which a twin differs from the real system is a bug
in the twin's configuration, or a reason to replace it.

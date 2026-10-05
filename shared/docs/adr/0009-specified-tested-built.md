# 0009. A requirement is specified, tested against twins, then built

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

A requirement said what must be true, but nothing checked how. Acceptance
criteria lived in issues, no test cited a requirement, and the write paths had
no design. Apprentices learn the way a feature is specified, so it must be the
same for every feature and easy to hold in the head.

jjforge depends on systems it doesn't deploy: the OpenID Connect provider each
organization connects, the object store, and Postgres. A requirement can pass
its tests and still fail against the running system, and `mise run up` can't
sign anyone in without a provider.

The options were:

- A spec-driven development tool such as Spec Kit or OpenSpec, or a tracing
  tool such as OpenFastTrace. Each brings its own tracker next to the issues,
  so a requirement is listed twice and the two lists drift.
- One spec page per epic in Markdown, with the scenarios as the acceptance,
  rendered on the docs site, and held to its form by the checks that already
  run.
- For the external systems: a mock in each test, which checks only what its
  author believed. Shared instances of the real systems, which tie every
  laptop and CI to the network and to accounts. Or a twin of each system: a
  local instance that speaks the same protocol, seeded with known data, and
  started with jjforge everywhere. Keycloak is slow to start and Dex serves
  one issuer, so `mock-oauth2-server` from NAV serves the issuers.

## Decision

- Every fact has one home. The issue tracks, the spec page describes, the
  scenario tests, and the contract declares. Nothing is restated.
- An epic is specified in one page, `shared/docs/specs/<epic number>-<slug>.md`,
  with one section per requirement, headed with its name and issue number, as
  in `## Create a repository (#25)`, and then the sections Flow, Persistence,
  Architecture, and Failures. [specs/README.md](../specs/README.md) gives the
  format, and `mise run prose` fails on a page without one of the four
  sections. A spec is written one epic ahead of its implementation, never
  more.
- A requirement's acceptance is its scenario: a Hurl file for REST, a Bats
  file for `jf`, with concrete values, linked from its section. A scenario
  whose operation still answers 501 lives in the `pending/` directory beside
  the others, and `mise run e2e` runs it without failing.
- The spec PR also changes the contract in `shared/api/` and `shared/proto/`
  until every assert is expressible. A scenario that needs something the
  contract lacks changes the contract, never the scenario.
- Every external system has a twin. It is taken off the shelf, and written
  here only when nothing conforms: Postgres itself, SeaweedFS for S3, and
  `mock-oauth2-server` for OpenID Connect. `shared/deploy/compose.yaml` runs
  every twin from the start, integration tests start the same images, and no
  integration test replaces an external system with a mock.
- A twin's seed is the test data: the organizations `acme` and `other`, each
  with its own issuer, and their owners and members, in
  `shared/deploy/twins/`. The Hurl fixtures name the same people. The
  Architecture section of a spec page names the external systems its paths
  call, and a new one brings its twin in the spec PR.
- A requirement is built in its own PR, by someone other than the spec's
  author, whose body says `Closes #<n>`. The Spec workflow fails the PR unless
  `#<n>` heads a section of a spec page on the base branch. The build PR moves
  the scenario out of `pending/` and changes nothing else in the spec. A
  scenario or a flow that can't be built as written is a gap, fixed in a spec
  PR.
- A requirement is built when its scenarios pass against the running system,
  built from the PR with its twins, and the stored state matches the
  Persistence section of its spec page.

## Consequences

A requirement has three states, each visible in the tree: unspecified,
specified with a pending scenario, and built. The requirement issue holds a
statement and nothing else. A requirement that no request or `jf` command can
observe says so in its section and names the test that checks it instead.
`mise run up` needs more memory and starts slower with every twin, and a
behavior in which a twin differs from the real system is a bug in the twin's
configuration, or a reason to replace it.

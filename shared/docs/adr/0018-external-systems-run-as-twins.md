# 0018. External systems run as twins wherever jjforge runs

Status: proposed, 2026-10-04. Deciders: jjforge maintainers.

## Context

jjforge depends on systems it doesn't deploy: the OpenID Connect provider
each organization connects, as [ADR 0004](0004-identity-and-tokens.md)
decides, and the object store and Postgres that the infra repository runs, as
[ADR 0010](0010-roles-and-deployment-modes.md) decides. A requirement can pass
its tests and still fail against the running system, and `mise run up` can't
sign anyone in without a provider.

The options were:

- A mock in each test. Fast, but it checks only what its author believed
  about the system, and the running forge still has no provider.
- Shared instances of the real systems. Real behavior, but every laptop and
  CI depend on the network and on accounts, and tests share state.
- A twin of each system: a local instance that speaks the same protocol,
  seeded with known data, and started with jjforge everywhere. It costs one
  container per system and its start time.

For OpenID Connect, Keycloak is a full provider, slow to start, and
configured through realms. Dex serves one issuer per instance.
`mock-oauth2-server` from NAV serves any number of issuers from one container
and reads its users from a JSON file.

## Decision

- Every external system has a twin. It is the same software where that runs
  locally, such as Postgres, or another implementation of the same protocol,
  such as SeaweedFS for S3 and `mock-oauth2-server` for OpenID Connect. A twin
  is taken off the shelf, and written here only when nothing conforms.
- Twins are part of the system. `shared/deploy/compose.yaml` runs every twin,
  integration tests start the same images, and CI runs `mise run smoke`
  against compose. No integration test replaces an external system with a
  mock.
- A twin's seed is the test data: the organizations `acme` and `other`, each
  with its own issuer, and their owners and members. The Hurl fixtures name
  the same people. `shared/deploy/twins/` holds the seeds.
- Compose runs every twin from the start, before any code calls it. A design
  names the external systems its write path calls, and a new one brings its
  twin in the PR that specifies it.
- A requirement is built when its scenarios pass against the running system,
  built from the PR with its twins, and the logs and stored state match the
  design. The `jjforge-implement` skill walks through that check.

## Consequences

`mise run up` needs more memory and starts slower with every twin. A behavior
in which a twin differs from the real system is a bug in the twin's
configuration, or a reason to replace the twin.

# 0015. Code rules are checked, not reviewed

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

Apprentices write most of the code, and a rule that only review holds is
applied unevenly. The server read its one setting with `@Value` and never
validated it, and vcsd parsed its address by hand. Formatting was the only
rule a tool checked.

The server's architecture rules could use Konsist, which reads Kotlin source,
or ArchUnit, which reads compiled classes. Konsist's latest release
predates Kotlin 2.4. ArchUnit doesn't depend on the Kotlin version, and Spring
Modulith builds on it.

## Decision

Every rule below fails the build. None has a baseline.

Server, in `gradle build`:

- Settings live in `@ConfigurationProperties` classes named `*Properties`,
  annotated `@Validated`, and constrained with Jakarta Validation, so an
  invalid setting stops the server at startup. Nothing reads `@Value`.
- A component gets its dependencies through its constructor. It has no
  `@Autowired` or `lateinit` field.
- `ArchitectureTest` checks both rules with ArchUnit. `ControllerContractTest`
  checks the controllers, as [ADR 0011](0011-controllers-implement-contracts.md)
  decides.
- `ModulesTest` runs Spring Modulith's verification: a module uses another only
  through its top-level package.
- detekt 2 checks complexity, naming, and likely bugs. ktlint formats.
- Compiler warnings are errors.
- Kover fails below 80 percent line coverage of the hand-written code.

vcsd and `jf`, in `mise run rust:lint`:

- Each binary reads its settings from flags or the environment into one clap
  struct. An invalid value stops it with a usage message.
- clippy's `pedantic` group is denied, and so are `unwrap` and `expect` outside
  tests. `unsafe` is forbidden.
- rustdoc warnings are errors.
- cargo-machete fails on an unused dependency, and cargo-deny on a
  disallowed license, a security advisory, or an unknown source.

The whole repository, in `mise run lint`: actionlint checks the workflows,
hadolint the Dockerfiles, and shellcheck the task scripts.

## Consequences

detekt 2 is still an alpha release. detekt 1.23 can't run on JDK 25, so the
alpha is the only choice, and a release candidate replaces it once one exists.

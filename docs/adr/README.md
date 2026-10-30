# Architecture decisions

Each record states one decision that shapes more than one component: its
options, its rules, and what follows.

- A new record starts from [the template](0000-template.md), is proposed in
  its own PR, and is accepted when the PR merges.
- Until 1.0.0, an accepted record is edited or deleted when its decision
  changes, because nothing stable depends on the old decision. Git history
  keeps the old wording.
- From 1.0.0 on, an accepted record is never edited. A new record
  supersedes it, and the old one's status names its successor.

```yaml table
- ADR: "[0001](0001-native-jj-without-git.md)"
  Decision: "Native jj without git: the format, the sync protocol, and `jf`"
  Status: Accepted
- ADR: "[0002](0002-state-boundaries-and-tokens.md)"
  Decision: Repositories and reviews in the object store, the rest in Postgres
  Status: Accepted
- ADR: "[0003](0003-checked-code-rules.md)"
  Decision: Code rules are checked, not reviewed
  Status: Accepted
- ADR: "[0004](0004-specified-tested-built.md)"
  Decision: A requirement is specified, tested against twins, then built
  Status: Accepted
- ADR: "[0005](0005-telemetry-through-opentelemetry.md)"
  Decision: Logs, traces, and metrics through OpenTelemetry
  Status: Accepted
- ADR: "[0006](0006-bounded-responses.md)"
  Decision: Every response is bounded in items, bytes, and time
  Status: Proposed
- ADR: "[0007](0007-one-identity-for-access-and-signatures.md)"
  Decision: One identity for access and signatures
  Status: Accepted
```

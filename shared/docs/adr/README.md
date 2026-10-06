# Architecture decisions

Each record states one decision that shapes more than one component: its
options, its rules, and what follows.

- A new record starts from [the template](0000-template.md), is proposed in
  its own PR, and is accepted when the PR merges.
- Until the first release, an accepted record is edited or deleted when its
  decision changes, because nothing shipped depends on the old decision.
- From the first release on, an accepted record is never edited. A new
  record supersedes it, and the old one's status names its successor.

| ADR                                         | Decision                                                       | Status   |
| ------------------------------------------- | -------------------------------------------------------------- | -------- |
| [0001](0001-native-jj-without-git.md)       | Native jj without git: the format, the sync protocol, and `jf` | Accepted |
| [0002](0002-state-boundaries-and-tokens.md) | Repositories in the object store, the product in Postgres      | Accepted |
| [0003](0003-checked-code-rules.md)          | Code rules are checked, not reviewed                           | Accepted |
| [0004](0004-specified-tested-built.md)      | A requirement is specified, tested against twins, then built   | Accepted |

# Planning

Work is planned in GitHub issues and the
[jjforge Project](https://github.com/orgs/nca-apprentices/projects/1). Durable
content lives in this repository.

## Model

| Layer        | Answers                                         | Lives in                                   | Checked by                 |
| ------------ | ----------------------------------------------- | ------------------------------------------ | -------------------------- |
| Initiative   | Why: the outcome and how success is measured    | Issue of type Initiative                   | Success measure            |
| Epic         | What can be demonstrated when it is done        | Issue of type Epic, under an initiative    | Demo and exit criteria     |
| Requirement  | What must be true, tested from the outside      | Issue of type Requirement, under an epic   | Acceptance tests           |
| ADR          | Which decision was made, and why                | [adr/](adr/README.md)                      | Review                     |
| Architecture | How the parts fit together                      | [architecture.md](architecture.md)         | Review                     |
| Spec         | Exactly how each interface behaves              | `api/`, `proto/`, [cli.md](cli.md)         | Lint, contract tests, Hurl |
| Concepts     | jj compared with git, and the UI consequences   | [concepts.md](concepts.md)                 | Review                     |
| Surfaces     | Where each capability appears on each interface | [surfaces.md](surfaces.md)                 | Review                     |

## Rules

1. **A requirement names no interface.** It says what a person can do and how
   that is tested. It never names an endpoint, a command, or a technology.
   Limits such as latency or size are allowed.
2. **A spec is exact and cites its requirements.** Each operation in
   `shared/api/` carries `@extension("x-requirements", #["#35"])`. Every test
   file, such as a CLI test or a Hurl file, starts with `satisfies #35`.
   `mise run spec:trace` shows which requirements are cited, and fails on a
   closed one that isn't.
3. **The issue number is the reference.** Specs, tests, and commits cite `#35`.
   The title prefix shows the type and the order: `I0` for an initiative, `E01`
   for an epic, `R001` for a requirement, and `T001` for a task. A new issue
   takes the next free ID of its type by hand.
4. **The contract comes first.** A spec may merge before its implementation. An
   operation that isn't built yet answers 501, as
   [ADR 0011](adr/0011-controllers-implement-contracts.md) decides.
5. **Status lives in issues, and durable content lives in the repository.** An
   issue holds no design text beyond its statement and acceptance criteria.
6. **ADRs change by the rules in ADR 0001.** Until the first release, an ADR
   is edited or deleted when its decision changes. After it, a new ADR
   supersedes the old one.

## Definition of done

| Level       | Done when                                                                                                                  |
| ----------- | -------------------------------------------------------------------------------------------------------------------------- |
| Requirement | Spec updated, tests merged and green, and its row in [surfaces.md](surfaces.md) filled for every surface the epic promises |
| Epic        | Demo shown and exit criteria hold                                                                                          |
| Initiative  | Success measure met, or cancelled with a stated reason                                                                     |

## Issue types and tracking

Issues use the types Initiative, Epic, Requirement, Task, and Bug. A task is
engineering work that no requirement states, such as moving the web app into
its own image. A requirement or a task is a sub-issue of its epic, and an epic
is a sub-issue of its initiative. The issue forms set the type.

The only label is `good first issue`, which GitHub lists on the repository's
contribute page. It marks a task or a requirement with clear steps and a
small scope.

The Project has two fields:

- **Status**, one of Todo, Doing, Review, or Done.
- **Target**, the date an initiative or an epic is due.

[close-epic.yml](../../.github/workflows/close-epic.yml) closes an epic once
all of its sub-issues are closed. An initiative is closed by hand, because its
success measure decides.

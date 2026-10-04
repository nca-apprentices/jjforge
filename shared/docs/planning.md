# Planning

Work is planned in GitHub issues and the
[jjforge Project](https://github.com/orgs/nca-apprentices/projects/1). Status
lives in issues, and durable content lives in the repository. An issue holds
no design text beyond its statement and acceptance criteria.

## Issues

| Type        | Answers                                      | Done when                                                                                                                  |
| ----------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Initiative  | Why: the outcome and how success is measured | Success measure met, or cancelled with a stated reason                                                                     |
| Epic        | What can be demonstrated when it is done     | Demo shown and exit criteria hold                                                                                          |
| Requirement | What must be true, tested from the outside   | Spec updated, tests merged and green, and its row in [surfaces.md](surfaces.md) filled for every surface the epic promises |

The other types are Task and Bug. A task is engineering work that no
requirement states, such as moving the web app into its own image. A
requirement or a task is a sub-issue of its epic, and an epic is a sub-issue
of its initiative. The issue forms set the type.

A [milestone](https://github.com/nca-apprentices/jjforge/milestones) is a
state of the forge, defined in its description. An epic or a task carries one
once it is planned. A requirement carries none, because its epic's covers it.

## Rules

1. **A requirement names no interface.** It says what a person can do and how
   that is tested. It never names an endpoint, a command, or a technology.
   Limits such as latency or size are allowed.
2. **A requirement is specified before it is built**, as
   [ADR 0017](adr/0017-specified-before-built.md) decides.
3. **The issue number is the reference.** Specs, tests, and commits cite `#35`.
   The title prefix shows the type and the order: `I0` for an initiative, `E01`
   for an epic, `R001` for a requirement, and `T001` for a task. A new issue
   takes the next free ID of its type by hand.

## Tracking

The only label is `good first issue`, which GitHub lists on the repository's
contribute page. It marks a task or a requirement with clear steps and a
small scope.

The Project tracks each issue's status. An epic's start and target dates
place it on the Project's roadmap, and its milestone's due date bounds them.

[close-epic.yml](../../.github/workflows/close-epic.yml) closes an epic once
all of its sub-issues are closed. An initiative is closed by hand, because its
success measure decides.

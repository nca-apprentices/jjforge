# Planning

Work is planned in GitHub issues and the
[jjforge Project](https://github.com/orgs/nca-apprentices/projects/1). Status
lives in issues, and durable content lives in the repository. An issue holds
no design text beyond its statement. The spec page of its epic, in
[specs/](specs/README.md), holds the rest.

## Issues

| Type        | Answers                                      | Done when                                                                        |
| ----------- | -------------------------------------------- | -------------------------------------------------------------------------------- |
| Epic        | What can be demonstrated when it is done     | Demo shown and exit criteria hold                                                |
| Requirement | What must be true, tested from the outside   | Its build PR merged with `Closes #n`, so its scenario left `pending/` and passes |

The other types are Task and Bug. A task is engineering work that no
requirement states, such as moving the web app into its own image. A
requirement or a task is a sub-issue of its epic. The issue forms set the
type.

A [milestone](https://github.com/nca-apprentices/jjforge/milestones) is a
state of the forge, defined in its description. An epic or a task carries one
once it is planned. A requirement carries none, because its epic's covers it.

## Rules

1. **A requirement names no interface.** It says what a person can do and how
   that is tested. It never names an endpoint, a command, or a technology.
   Limits such as latency or size are allowed.
2. **A requirement is specified before it is built**, as
   [ADR 0009](adr/0009-specified-tested-built.md) decides. One page per epic
   in [specs/](specs/README.md) holds a section per requirement.
   [Development](development.md#specifying-and-building) lists the steps.
3. **The issue number is the reference.** A spec section, a build PR, and a
   commit cite `#35`. A title holds no ID, because the number and the type
   already identify the issue.

## Tracking

The only label is `good first issue`, which GitHub lists on the repository's
contribute page. It marks a task or a requirement with clear steps and a
small scope.

The Project tracks each issue's status. An epic's start and target dates
place it on the Project's roadmap, and its milestone's due date bounds them.

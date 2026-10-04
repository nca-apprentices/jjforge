# 0017. A requirement is specified before it is built

Status: accepted, 2026-10-04. Deciders: jjforge maintainers.

## Context

The contract comes first, and checks hold it to that. The behavior behind it
had no check: acceptance criteria lived only in issues, no test cited a
requirement, and the kernel streams of [ADR 0005](0005-storage-kernel.md)
had no event schema. A contract says what an operation accepts and answers,
not what it does, and not what a crash leaves behind. Apprentices learn the way a
feature is specified, so it must be the same for every feature.

The options were:

- A spec-driven development tool such as Spec Kit or OpenSpec. Its
  constitution repeats the ADRs, and its plan and tasks repeat the epics, so
  a third planning system sits next to the issues and the ADRs.
- Design documents alone. Nothing checks that they exist or stay current.
- Scenarios as tests before the contract, and a design per epic for the
  write paths, both citing issue numbers so the trace already covers them.

## Decision

- A requirement is specified in its own PR, before anyone builds it, and a
  different person builds it. A question the builder has to ask is a gap in
  the spec, fixed in the spec.
- Scenarios are tests with concrete values, starting with `satisfies #35`.
  While the operation answers 501 they are pending: a Hurl file under
  `shared/http/pending/`, which `mise run smoke` runs without failing, or a
  Rust test marked `#[ignore = "pending #35"]`. The PR that builds the
  operation moves the file or drops the mark, and changes nothing else.
- The contract is reviewed against the scenarios. A scenario that needs
  something the contract lacks changes the contract, never the scenario.
- A write path has a design: one file per epic in
  [design/](../design/README.md), one section per write path starting with
  `designs #35`, and its event schema as a proto in the same PR.
- `mise run spec:trace` lists designs next to operations and tests, and
  fails on a closed requirement that a write operation cites and no design
  covers.
- A spec is written one epic ahead of its implementation, never an
  initiative ahead.

## Consequences

A requirement has a checkable state between opened and done: specified, when
its scenarios run red against a merged contract and its design has landed
with its proto.

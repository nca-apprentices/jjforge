# 0017. A requirement is specified before it is built

Status: accepted, 2026-10-04. Deciders: jjforge maintainers.

## Context

The contract comes first, as [planning](../planning.md) and
[ADR 0011](0011-controllers-implement-contracts.md) decide, and the checks
hold it to that: TypeSpec compiles, Redocly lints, oasdiff diffs, and
`spec:trace` fails on an operation that cites no requirement. The behavior
behind the contract had no such check. A requirement's acceptance criteria
lived only in its issue, outside the repository, no test cited a requirement,
and the kernel streams of [ADR 0005](0005-storage-kernel.md) had no event
schema anywhere. A contract says what an operation accepts and answers, not
what it does, in which order, or what a crash leaves behind.

jjforge is also a training program, so the way a feature is specified is
itself something apprentices learn, and it must be the same for every feature.

The options were:

- A spec-driven development tool, such as Spec Kit or OpenSpec, with a
  constitution, a plan, tasks, and specs in Markdown. The constitution
  repeats the ADRs, the plan and tasks repeat the epics and tasks, and a
  third planning system sits next to the issues and the ADRs.
- Design documents alone. They say how a feature works, but nothing checks
  that they exist, that they match the code, or that they stay current.
- Scenarios as tests, in the repository, before the contract, and a design
  per epic for the write paths, both citing requirements by issue number so
  the trace already in place covers them.

## Decision

- A requirement is specified in its own PR, before anyone builds it. The spec
  PR holds the scenarios, the contract change if the scenarios need one, and
  the design section if the requirement changes state. A different person
  implements it, and every question the implementer has to ask is a gap in
  the spec, fixed in the spec.
- Scenarios are tests with concrete values, one file per surface the epic
  promises, starting with `satisfies #35`. A scenario whose operation still
  answers 501 is pending: a Hurl file under `shared/http/pending/`, or a Rust
  test marked `#[ignore = "pending #35"]`. `mise run smoke` runs the pending
  Hurl files and reports them without failing. The implementation PR moves
  or removes the ignore mark and changes nothing else in it.
- The contract is reviewed against the scenarios: every assert must be
  expressible with it. A scenario that needs a field the contract lacks
  changes the contract, not the scenario.
- A write path has a design. Each epic has one design in
  [design/](../design/README.md), linked from the epic's Design field, and
  each section cites the requirements it serves on a `designs #35` line. A
  design names the policy, the claimed names, the streams and events, the
  read model, and the partial failures. Its event schema is a proto file in
  the same PR, under `buf breaking` like every other contract.
- `mise run spec:trace` lists the designs next to the operations and the
  tests that cite a requirement, and it fails on a closed requirement that
  a write operation cites and no design covers.
- A spec is written one epic ahead of its implementation, never an
  initiative ahead.

## Consequences

- A requirement has a checkable state between opened and done: specified,
  when its scenarios run red against a merged contract and its design, if it
  needs one, has landed with its proto.
- A design that produced no proto and no projector test is prose, and the
  trace doesn't count it.
- The first specs cost more than the features they describe. That is the
  point of a training program.

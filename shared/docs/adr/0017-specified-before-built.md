# 0017. A requirement is specified before it is built

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

A requirement said what must be true, but nothing checked how. Acceptance
criteria lived in issues, no test cited a requirement, and the write paths had
no design. The first answer, a spec per requirement with a trace script, put
the same list of requirements in the issue, the scenario, the contract, a
design file, and an opt-out file, and a script compared them. Apprentices
learn the way a feature is specified, so it must be the same for every feature
and easy to hold in the head.

The options were:

- A spec-driven development tool such as Spec Kit or OpenSpec. Each brings its
  own tracker, a plan or a tasks file, next to the issues, so a requirement is
  listed twice and the two lists drift.
- A requirements tracing tool such as OpenFastTrace. Its items replace the
  requirement issues, and its coverage tags live only in the file types it
  scans, which the scenario files aren't.
- One spec page per epic in Markdown, with the scenarios as the acceptance,
  rendered on the docs site, and held to its form by the checks that already
  run.

## Decision

- Every fact has one home. The issue tracks, the spec page describes, the
  scenario tests, and the contract declares. Nothing is restated.
- An epic is specified in one page, `shared/docs/specs/<epic number>-<slug>.md`.
  The page has one section per requirement, headed with the requirement's
  name and issue number as in `## Create a repository (#25)`, then the
  sections Flow, Persistence, Architecture, and Failures. A requirement's
  section merges in a spec PR before the requirement is built, and the four
  sections grow with it.
  [specs/README.md](../specs/README.md) gives the format, and
  `mise run prose` fails on a page without one of the four sections.
- A requirement's acceptance is its scenario: a Hurl file for REST, a Bats
  file for `jf`, with concrete values, linked from its section. A scenario
  whose operation still answers 501 lives in the `pending/` directory beside
  the others, and `mise run e2e` runs it without failing.
- The spec PR also changes the contract in `shared/api/` and `shared/proto/`
  until every assert is expressible. A scenario that needs something the
  contract lacks changes the contract, never the scenario.
- A requirement is built in its own PR, by someone other than the spec's
  author, whose body says `Closes #<n>`. The Spec workflow fails the PR unless
  `#<n>` heads a section of a spec page on the base branch. The build PR moves
  the scenario out of `pending/`, updates its link, and changes nothing else
  in the spec. A
  scenario or a flow that can't be built as written is a gap, fixed in a spec
  PR.
- A spec is written one epic ahead of its implementation, never more.

## Consequences

A requirement has three states, each visible in the tree. Unspecified, when no
section heads with its number. Specified, when the section exists and its
scenario is pending. Built, when the scenario has left `pending/` and the
issue is closed. The requirement issue holds a statement and nothing else,
because the section holds the rest.

A requirement that no request or `jf` command can observe says so in its
section and names the test that checks it instead.

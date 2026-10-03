# 0001. Record architecture decisions

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

jjforge is also a training program, so people join throughout the year. A
decision that lives only in a chat or a meeting gets argued again by each new
person, and the reasons behind the code get lost.

Options: no record, a wiki, decisions in issues, or decision records in the
repository. A wiki and issues drift away from the code they describe, and
issues hold status, not durable content.

## Decision

- Every decision that shapes more than one component gets an architecture
  decision record (ADR) in `shared/docs/adr/`. ADRs are numbered in order
  and start from [the template](0000-template.md).
- An ADR is proposed in its own PR and accepted when the PR merges.
- Until the first release, an accepted ADR is edited or deleted in a PR when
  its decision changes, and the index follows. Nothing has shipped that
  depends on the old decision.
- From the first release on, an accepted ADR is never edited. A new ADR
  supersedes it, and the old one's status names its successor.
- The [index](README.md) lists every ADR with its status.

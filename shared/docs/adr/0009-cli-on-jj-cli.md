# 0009. The CLI is `jf`, built on jj-cli

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

People and agents drive jjforge from a terminal. A separate forge tool next to
jj splits their attention, and a copy of jj's commands would drift from jj.

Options: a standalone client next to jj, a jj alias, or a binary built on
jj-cli's `CliRunner`, which adds commands to the full set of jj commands. For
the name: `jjforge`, `jjf`, `forge`, or `jf`.

## Decision

- The CLI is built on jj-cli's `CliRunner`. Every jj command works in it, and
  it adds the forge commands.
- It is called `jf`. People type it constantly, so the shortest name wins. It
  clashes with the JFrog CLI binary, and that cost is accepted. A person who
  has both installs one under another name.
- `jf` talks to the forge only through the public API, as
  [ADR 0002](0002-public-api-is-http.md) decides.
- The CLI is specified in four layers:
  1. Conventions, in [cli.md](../cli.md).
  2. Reference, which CI generates from clap and checks for drift.
  3. Behavior, as trycmd scenarios that each cite their requirement.
  4. JSON, which is exactly the OpenAPI schema for the same data.

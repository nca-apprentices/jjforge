# 0009. The CLI is `jf`, built on jj-cli

Status: accepted, 2026-10-03. Deciders: jjforge maintainers.

## Context

People and agents drive jjforge from a terminal. A separate forge tool next to
jj splits their attention, and a copy of jj's commands would drift from jj.
jjforge stores repositories in its own format, as
[ADR 0007](0007-native-jj-without-git.md) decides, so plain jj can't open
them.

Options: a standalone client next to jj, a jj alias, or a binary built on
jj-cli's `CliRunner`, which adds commands to the full set of jj commands and
accepts store factories. For the name: `jjforge`, `jjf`, `forge`, or `jf`.

## Decision

- The CLI is built on jj-cli's `CliRunner`. Every jj command works in it, and
  it adds the forge commands.
- It registers the jjforge implementations of `Backend`, `OpStore`, and
  `OpHeadsStore`, so it opens jjforge repositories and plain jj doesn't.
- It is called `jf`. People type it constantly, so the shortest name wins. It
  clashes with the JFrog CLI binary, and that cost is accepted. A person who
  has both installs one under another name.
- A clone fetches the operations and views, and the working copy fetches files
  and trees as it needs them.
- `jf` talks to the forge through the REST API and the sync protocol, and
  nothing else, as [ADR 0002](0002-public-protocols.md) decides.
- `jf` links jj-lib with its git support, so a person can still use `jf git`
  against other hosts, and `jf import` can read a repository that jj keeps on
  git.
- The CLI is specified in four layers:
  1. Conventions, in [`cli.md`](../cli.md).
  2. Reference, which CI generates from clap and checks for drift.
  3. Behavior, as trycmd scenarios that each cite their requirement.
  4. JSON, which is exactly the OpenAPI schema for the same data.

## Consequences

Each `jf` release pins one jj version, so `jf` follows jj's releases with a
delay.

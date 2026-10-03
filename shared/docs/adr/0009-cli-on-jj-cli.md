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
  `OpHeadsStore` under the store type `jjforge`, next to jj's own, so it opens
  jjforge repositories and plain jj doesn't.
- It is called `jf`. People type it constantly, so the shortest name wins. It
  clashes with the JFrog CLI binary, and that cost is accepted. A person who
  has both installs one under another name.
- A clone fetches the operations and views, and the working copy fetches files
  and trees as it needs them.
- `jf` talks to the forge through the REST API and the sync protocol, and
  nothing else, as [ADR 0002](0002-public-protocols.md) decides.
- `jf` links jj-lib with its git support. In a repository that jj keeps on
  git, `jf git` works as `jj git` does, and `jf import` can read it. In a
  jjforge repository, `jf git` reports that the repository isn't backed by
  git, because jj-lib's git commands need the git backend.
- The CLI is specified in four layers:
  1. Conventions, in [`cli.md`](../cli.md).
  2. Reference, which CI generates from clap and checks for drift.
  3. Behavior, as trycmd scenarios that each cite their requirement.
  4. JSON, which is exactly the OpenAPI schema for the same data.

## Consequences

Each `jf` release pins one jj version, so `jf` follows jj's releases with a
delay.

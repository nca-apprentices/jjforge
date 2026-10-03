# 0017. jjforge stores and syncs jj data natively, without git

Status: accepted, 2026-10-03. Deciders: jjforge maintainers. Supersedes
[ADR 0007](0007-git-bridge-first.md) and [ADR 0009](0009-cli-on-jj-cli.md).

## Context

jj works on git today because its only remote is git. Git can't hold what makes
jj worth using: a change ID survives only as an extra commit header that git
tools drop, a conflict is encoded into ordinary trees, and the operation log
stays on each machine. A forge on git keeps translating between two models.

jj-lib separates the backend from its logic. `Backend`, `OpStore`, and
`OpHeadsStore` are traits, and the git backend is one implementation of them.
jj-cli's `CliRunner` accepts store factories, so a binary built on it can
register a backend of its own.

The options were:

- Serve git smart HTTP first and add a native protocol later, as ADR 0007
  decided. Every jj and git client works at once, but the forge carries both
  models and the native protocol keeps slipping.
- Keep the git format in storage and sync natively. The protocol is native,
  but the change ID and conflicts still live in a format that wasn't made for
  them.
- Go native in storage, in the protocol, and in the client. The model is
  clean, but only `jf` talks to jjforge, and tools that expect git can't.

## Decision

- jjforge has no git in it: no git object format, no git smart HTTP, and no
  colocated repositories.
- A repository holds six kinds of objects, encoded with Protocol Buffers and
  named by the BLAKE3 hash of their bytes:
  1. File: the content.
  2. Symbolic link: the target.
  3. Tree: sorted entries, each a name pointing at a file, a symbolic link, or
     a tree, with an executable bit for a file.
  4. Commit: a change ID, parents, the root tree, author, committer,
     description, and an optional signature. The root tree of a conflicted
     commit is a merge of trees.
  5. Operation: parents, a view, the principal and its delegation chain, a
     time, and a description.
  6. View: the visible heads and the bookmarks. A bookmark points at one
     target, or at several when it is conflicted.
- The change ID is a field of the commit, and a conflict is a merge of trees
  in the commit. Neither is encoded into anything else.
- The CLI is `jf`, built on jj-cli's `CliRunner`. Every jj command works in
  it, and it adds the forge commands. It is called `jf` because people type it
  constantly. It clashes with the JFrog CLI binary, and that cost is accepted.
- `jf` registers the jjforge implementations of `Backend`, `OpStore`, and
  `OpHeadsStore`. The same crate serves vcsd, so the format has one
  implementation.
- `jf` reads files and trees on demand. A clone fetches the operations and
  views, and the working copy fetches what it needs.
- `jf` talks to the forge through the REST API and the sync protocol, as
  [ADR 0019](0019-public-protocols.md) decides.
- The backend also works without a forge, pointed at a bucket, as in
  `jf init --store s3://bucket/repo`. Compare-and-swap on the head object
  keeps concurrent writers safe, and only people trusted with the bucket
  can write.
- `jf import` copies the history of an existing jj repository into a jjforge
  repository through jj-lib's traits, whatever backend the source uses. It
  runs on the person's machine, and the forge never reads git.
- The CLI is specified in four layers:
  1. Conventions, in [`cli.md`](../cli.md).
  2. Reference, which CI generates from clap and checks for drift.
  3. Behavior, as trycmd scenarios that each cite their requirement.
  4. JSON, which is exactly the OpenAPI schema for the same data.

## Consequences

- Plain `jj` and `git` clients can't clone from or push to jjforge. Each
  person installs `jf`.
- Tools that fetch from git, such as hosted CI, IDE integrations, and package
  managers that resolve git dependencies, don't work against jjforge.
  jjforge's own sandboxes use `jf`, and vcsd serves archives of a revision
  for tools that only need files.
- jj-lib's traits change between releases. Each `jf` release pins one jj
  version, and the backend runs jj's backend tests.
- `jf` links jj-lib with its git support, so a person can still use
  `jf git` against other hosts and `jf import` can read a repository that jj
  keeps on git.

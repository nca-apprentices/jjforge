# 0007. jjforge stores and syncs jj data natively, without git

Status: accepted, 2026-10-03. Deciders: jjforge maintainers.

## Context

jj works on git today because its only remote is git. Git can't hold what makes
jj worth using: a change ID survives only as an extra commit header that git
tools drop, a conflict is encoded into ordinary trees, and the operation log
stays on each machine. A forge on git keeps translating between two models.

jj-lib separates the backend from its logic. `Backend`, `OpStore`, and
`OpHeadsStore` are traits, and the git backend is one implementation of them.
jj-lib also ships `SimpleBackend`, a proof of concept with the same shapes in
Protocol Buffers, and jj-cli's `custom-backend` example registers a backend
through `CliRunner`'s store factories. Google runs its own backend behind the
same traits.

The options were:

- Serve git smart HTTP first and add a native protocol later. Every jj and git
  client works at once, but the forge carries both models and the native
  protocol keeps slipping.
- Keep the git format in storage and sync natively. The protocol is native,
  but the change ID and conflicts still live in a format that wasn't made for
  them.
- Go native in storage, in the protocol, and in the client. The model is
  clean, but only `jf` talks to jjforge, and tools that expect git can't.

## Decision

- jjforge has no git in it: no git object format, no git smart HTTP, and no
  colocated repositories.
- A repository holds six kinds of objects, encoded with Protocol Buffers and
  named by the BLAKE3 hash of their bytes, as
  [proto/store/v1](../../proto/store/v1/) describes:
  1. File: the content.
  2. Symbolic link: the target.
  3. Tree: sorted entries, each a name pointing at a file, a symbolic link, or
     a tree, with an executable bit for a file.
  4. Commit: a change ID, parents, the root tree, author, committer,
     description, and an optional signature. The root tree of a conflicted
     commit is a merge of trees, with a label for each term.
  5. Operation: parents, a view, a time, a description, and the attributes
     jj keeps on an operation. The forge records the principal and its
     delegation chain as attributes, from the verified token.
  6. View: the visible heads, the bookmarks, and the tags. A bookmark points
     at one target, or at several when it is conflicted.
- The change ID is a field of the commit, and a conflict is a merge of trees
  in the commit. Neither is encoded into anything else.
- The format holds every field of jj-lib's `Commit`, `View`, and `Operation`
  except the git ones, so a repository round-trips through jj-lib without
  loss.
- One Rust crate implements `Backend`, `OpStore`, and `OpHeadsStore` for this
  format. vcsd and `jf` both use it, so the format has one implementation.
  The head moves by compare-and-swap, which `OpHeadsStore` allows: its lock
  is optional, and jj-lib merges divergent heads itself.
- The backend also works without a forge, pointed at a bucket, as in
  `jf init --store s3://bucket/repo`. Compare-and-swap on the head object
  keeps concurrent writers safe, and only people trusted with the bucket can
  write.
- `jf import` copies the history of an existing jj repository into a jjforge
  repository through jj-lib's traits, whatever backend the source uses. It
  runs on the person's machine, and the forge never reads git.

## Consequences

- Plain `jj` and `git` clients can't clone from or push to jjforge. Each
  person installs `jf`, as [ADR 0009](0009-cli-on-jj-cli.md) describes.
- Tools that fetch from git, such as hosted CI, IDE integrations, and package
  managers that resolve git dependencies, don't work against jjforge.
  jjforge's own sandboxes use `jf`, and vcsd serves archives of a revision
  for tools that only need files.
- jj-lib's traits change between releases. Each `jf` release pins one jj
  version. jj-lib publishes no conformance tests for a backend, and its test
  utilities aren't published, so the crate carries tests of its own, ported
  from jj-lib's.
- jj-lib has no hook to fetch ahead of a checkout. A checkout reads each file
  on its own, with as many requests in flight as the backend declares. `jf`
  fetches the objects of the target tree in bundles before it hands a checkout
  to jj-lib.
- `jf import` writes new commits, so every commit ID changes and a signature
  made over the old format doesn't carry over. Change IDs stay.
- A stored object never changes, so a field of the format is only ever added.

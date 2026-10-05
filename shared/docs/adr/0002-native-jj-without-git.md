# 0002. Native jj without git: the format, the sync protocol, and `jf`

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

jj works on git today because its only remote is git. Git can't hold what makes
jj worth using: a change ID survives only as an extra commit header that git
tools drop, a conflict is encoded into ordinary trees, and the operation log
stays on each machine. A forge on git keeps translating between two models.

jj-lib separates the backend from its logic. `Backend`, `OpStore`, and
`OpHeadsStore` are traits, and the git backend is one implementation of them.
jj-cli's `CliRunner` adds commands to the full set of jj commands and accepts
store factories, so a binary built on it opens a backend that plain jj can't.

The options were:

- Serve git smart HTTP first and add a native protocol later. Every client
  works at once, but the forge carries both models and the native protocol
  keeps slipping.
- Keep the git format in storage and sync natively. The change ID and
  conflicts still live in a format that wasn't made for them.
- Go native in storage, in the protocol, and in the client. The model is
  clean, but only the forge's own client can reach it.

Moving objects differs from every other operation: objects are immutable and
named by their hash, uploads are large, and the head moves only when
everything it references has arrived. Sync operations in the REST contract
would pass every byte through the server, and HTTP caches can't cache gRPC.

## Decision

- jjforge has no git in it: no git object format, no git smart HTTP, and no
  colocated repositories.
- A repository holds six kinds of objects, encoded with Protocol Buffers and
  named by the BLAKE3 hash of their bytes, as
  [proto/store/v1](../../proto/store/v1/) describes: file, symbolic link,
  tree, commit, operation, and view. The change ID is a field of the commit,
  and a conflict is a merge of trees in the commit. The operation records
  the principal and its delegation chain, from the verified token. The
  format holds every field of jj-lib's `Commit`, `View`, and `Operation`
  except the git ones, so a repository round-trips without loss.
- One Rust crate implements `Backend`, `OpStore`, and `OpHeadsStore` for this
  format. vcs and `jf` both use it. The head moves by compare-and-swap,
  which `OpHeadsStore` allows, and jj-lib merges divergent heads itself.
- The public API is HTTP only, in two parts. REST, plus server-sent events,
  served by the server under `/api/v1/` and described in
  [shared/api](../../api/), with no private endpoints. A version changes
  only by addition, and a breaking change starts `/api/v2/` next to it. The
  OAuth routes stay where their RFCs put them. And `sync/v1`, served by vcs
  under `/sync/v1/` on the same host, with the messages and routes of
  [proto/sync/v1](../../proto/sync/v1/). It addresses a repository by its
  organization and repository IDs, so a rename doesn't break a clone.
- Objects are read with `GET`, and their answers are immutable. vcs checks the
  token before it serves or redirects to a short-lived signed URL, and
  verifies the hash of every uploaded object.
- A publish names the head it expects. vcs moves the head only when every
  referenced object is present, the token's scope covers each changed
  bookmark, a protected bookmark moves only under the landing token, and the
  change ownership policy holds. vcs decides all of this from the token.
- Both parts answer errors as `application/problem+json` with the same
  problem codes, so the CLI maps them to the same exit codes. gRPC runs only
  inside the cluster, under `source/v1`.
- The CLI is `jf`, built on jj-cli's `CliRunner`. Every jj command works in
  it, and it registers the jjforge stores under the store type `jjforge`. It
  is the shortest name, and the clash with the JFrog CLI is accepted. `jf`
  talks to the forge through REST and `sync/v1`, and nothing else.
- A clone fetches the operations and views. The working copy fetches files
  and trees as it needs them, in bundles, before `jf` hands a checkout to
  jj-lib.
- `jf import` copies the history of an existing jj repository into a jjforge
  repository through jj-lib's traits, on the person's machine. The forge
  never reads git.
- The CLI is specified in four layers: conventions in [`cli.md`](../cli.md),
  a reference that CI generates from clap, trycmd scenarios that cite their
  requirement, and JSON that is exactly the OpenAPI schema for the same data.

## Consequences

- Plain `jj` and `git` can't reach jjforge, and neither can tools that fetch
  from git. Each person installs `jf`, the forge's own sandboxes use `jf`,
  and vcs serves archives of a revision for tools that only need files.
- Each `jf` release pins one jj version. jj-lib publishes no conformance
  tests for a backend, so the crate carries its own, ported from jj-lib's.
- `jf import` writes new commits, so every commit ID changes and a signature
  over the old format doesn't carry over. Change IDs stay.
- A stored object never changes, so a field of the format is only ever added.
- `sync/v1` isn't in TypeSpec, because the server doesn't serve it. `buf
  breaking` guards it as it guards the internal contracts.

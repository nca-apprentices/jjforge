# 0001. Native jj without git: the format, the sync protocol, and `jf`

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

Git can't hold what makes jj worth using: a change ID survives only as a
commit header that git tools drop, a conflict is encoded into ordinary trees,
and the operation log stays on each machine. A forge on git translates
between two models forever. jj-lib makes `Backend`, `OpStore`, and
`OpHeadsStore` traits, and jj-cli's `CliRunner` accepts store factories, so a
binary built on it opens a backend that plain jj can't.

| Option                                        | Cost                                                           |
| --------------------------------------------- | -------------------------------------------------------------- |
| Git smart HTTP first, a native protocol later | The forge carries both models, and the native protocol slips   |
| Git format in storage, native sync            | The change ID and conflicts live in a format not made for them |
| Native storage, protocol, and client          | Only the forge's own client can reach it. Chosen               |

Moving objects differs from every other operation: objects are immutable and
named by their hash, uploads are large, and the head moves only when
everything it references has arrived. Sync in the REST contract would pass
every byte through the server, and HTTP caches can't cache gRPC.

## Decision

- jjforge has no git in it: no git object format, no git smart HTTP, and no
  colocated repositories.
- A repository holds six kinds of objects, encoded with Protocol Buffers and
  named by the BLAKE3 hash of their bytes, as
  [proto/store/v1](../../proto/store/v1/) describes: file, symbolic link,
  tree, commit, operation, and view. The change ID is a field of the commit,
  a conflict is a merge of trees, and the operation records the principal
  and its delegation chain from the verified token. The format holds every
  field of jj-lib's `Commit`, `View`, and `Operation` except the git ones.
- One Rust crate implements the three traits for this format, for vcs and
  `jf` alike. The head moves by compare-and-swap, and jj-lib merges divergent
  heads itself.
- The public API is HTTP only, in two parts. Both answer errors as
  `application/problem+json` with the same problem codes, so the CLI maps
  them to the same exit codes. gRPC runs only inside the cluster, under
  `source/v1`.

| Part      | Served by | Under       | Described in                               | Changes                                                                                   |
| --------- | --------- | ----------- | ------------------------------------------ | ----------------------------------------------------------------------------------------- |
| REST, SSE | server    | `/api/v1/`  | [shared/api](../../api/), no private route | Only by addition. A breaking change starts `/api/v2/` next to it                          |
| `sync/v1` | vcs       | `/sync/v1/` | [proto/sync/v1](../../proto/sync/v1/)      | Under `buf breaking`. It addresses a repository by IDs, so a rename doesn't break a clone |

- OAuth routes stay where their RFCs put them.
- Objects are read with `GET`, and their answers are immutable. vcs checks
  the token before it serves or redirects to a short-lived signed URL, and
  verifies the hash of every uploaded object.
- A publish names the head it expects. vcs moves the head only when every
  referenced object is present, the token's scope covers each changed
  bookmark, a protected bookmark moves only under the landing token, and the
  change ownership policy holds. vcs decides all of this from the token.
- The CLI is `jf`, built on jj-cli's `CliRunner`: every jj command works in
  it, and it registers the jjforge stores under the store type `jjforge`. The
  clash with the JFrog CLI is accepted. `jf` talks to the forge through REST
  and `sync/v1`, and nothing else.
- A clone fetches the operations and views. The working copy fetches files
  and trees as it needs them, in bundles, before `jf` hands a checkout to
  jj-lib.
- `jf import` copies the history of an existing jj repository through
  jj-lib's traits, on the person's machine. The forge never reads git.
- The CLI is specified in four layers: conventions in [`cli.md`](../cli.md),
  a reference that CI generates from clap, trycmd scenarios that cite their
  requirement, and JSON that is exactly the OpenAPI schema for the same data.

## Consequences

- Plain `jj`, `git`, and tools that fetch from git can't reach jjforge. Each
  person installs `jf`, the forge's sandboxes use `jf`, and vcs serves
  archives of a revision for tools that only need files.
- Each `jf` release pins one jj version. jj-lib publishes no conformance
  tests for a backend, so the crate carries its own, ported from jj-lib's.
- `jf import` writes new commits, so every commit ID changes and a signature
  over the old format doesn't carry over. Change IDs stay.
- A stored object never changes, so a field of the format is only ever added.

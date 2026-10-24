# 0006. Every response is bounded

Status: proposed, 2026-10-24. Deciders: jjforge maintainers.

## Context

A repository grows without limit: its history, the files of one change, and
the size of one file. A response that grows with it costs vcs memory and time
without limit, and a gRPC client refuses a message over its default limit
of 4 MiB. `GetChange` listed every file of a change, so 5,000 long paths
took its response past that limit. `Log` stopped at 1,000 changes, and a
client had no way to read on.

[ADR 0002](0002-state-boundaries-and-tokens.md) pages the lists of the REST
contract. It says nothing of `source/v1`, of bytes, or of time.

| Option                                         | Cost                                                                                         |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Answer everything, and raise the message limit | Memory and time grow with the repository, and every client raises its limit too              |
| Cap each list, and drop the rest               | A large change loses files without a trace                                                   |
| Page by a count of items                       | A page of long items still passes the message limit                                          |
| Bound items, bytes, and time                   | Each list takes a token, a client follows it, and a cut item needs a call of its own. Chosen |

GitHub answers the same forces in the same way. A commit lists its files
300 to a page, a tree stops at 100,000 entries and says it is truncated, a
diff too large to show is left out with a note, and a diff that takes too
long to compute says so.

## Decision

The cost of a response doesn't grow with the repository. Each response is
bounded in items, in bytes, and in time.

### Lists

- A list answers one page and a token for the next, which is empty on the
  last page. A client treats the token as opaque.
- A `source/v1` token names the operation that the first page read at.
  Every later page reads at that operation, so a push between two pages
  neither skips nor repeats an item.
- A page holds at most its page size and at most 3 MiB. Zero asks for the
  default page size.
- A list that streams, such as `Log` or `Diff`, streams one page, one
  message per item. The last message carries the token of the next page,
  and the request takes it back as `page_token`.
- A call whose client always needs the start of a list answers its first
  page inline, with the token that continues it in a list call of its own.
  `GetChange` answers the first page of a change's files, and
  `ListChangedFiles` answers the rest. The REST `Change` does the same with
  `filesNext` and `listChangedFiles`.
- A list bounded by its nature, such as the parents of a commit, carries a
  comment that starts with `Bounded:` and says why.

### Content

- A file's bytes stream in chunks of 64 KiB, and a read can name a range.
- An item that a page can't hold whole is cut, says so, and names the call
  that reads it whole. A diff of a file whose hunks take more than 1 MiB, or
  whose side is larger than 4 MiB, is `truncated`, and `ReadFile` reads
  each side.

### Time

- Every vcs call runs under a deadline of 30 seconds. Past it, vcs answers
  `DEADLINE_EXCEEDED`, which its telemetry counts as a failure of vcs, not
  of the caller. A client's shorter deadline ends the call on its own side.
- Work that grows with a diff has a limit of its own, as `diff.renameLimit`
  does in git. A diff looks for renames among at most 1,000
  added and 1,000 deleted files.
- A stream that waits for news, such as `WatchOperations`, runs without a
  deadline. Its cost per message stays bounded.

### Clients

`jf` and the web app follow tokens as a reader moves on, render a diff file
by file as it streams, and offer a cut item's full read as an action. A cut
is never silent.

### Checks

- `mise run shared:lint` fails on a `repeated` field of a `source/v1`
  message other than a request, at any depth, that has no page token
  beside it and no `Bounded:` comment.
- Redocly's rules `list-answers-a-page` and `page-shape` hold the REST
  contract, as [ADR 0003](0003-checked-code-rules.md) decides.

## Consequences

- `sync/v1` already holds the rule. A request names at most 1,000 objects,
  and a body takes at most 64 MiB.
- A deadline stops the answer, not the work that a blocking thread already
  runs. The limits of this record keep that work bounded.

# Concepts

jjforge shows Jujutsu (jj) repositories. Most people arrive knowing git, so
this page maps each jj concept to its closest git equivalent, says where the
analogy breaks, and says what that means for the UI. The
[jj documentation](https://jj-vcs.github.io/jj/latest/git-comparison/) covers
the concepts in depth.

```yaml table
- jj concept: Change
  Closest git equivalent: Commit
  Where the analogy breaks: >
    A change keeps its identity when it is rewritten. Each rewrite produces
    a new commit.
  What it means for the UI: >
    The change is the unit the UI shows, links to, and reviews. There is no
    pull request object.
- jj concept: Change ID
  Closest git equivalent: Commit hash
  Where the analogy breaks: >
    The change ID is stable across rewrites. The commit ID changes with
    every rewrite.
  What it means for the UI: >
    Change IDs lead and link. Commit IDs are secondary detail.
- jj concept: Working-copy commit
  Closest git equivalent: Index and working tree
  Where the analogy breaks: >
    jj records the working copy as a commit. There is no staging area.
  What it means for the UI: >
    The server has no working copy, so the UI never shows `@`.
- jj concept: Bookmark
  Closest git equivalent: Branch
  Where the analogy breaks: >
    A bookmark doesn't move when a new change is made on top of it. It moves
    only when set or pushed.
  What it means for the UI: >
    Call it a bookmark, never a branch. Show where each one points.
- jj concept: Trunk
  Closest git equivalent: "`main`"
  Where the analogy breaks: >
    `trunk()` is a revset that names the main bookmark, not a special
    branch.
  What it means for the UI: >
    Trunk is the default revision and the landing target.
- jj concept: Operation log
  Closest git equivalent: Reflog
  Where the analogy breaks: >
    The operation log records every change to the whole repository, and any
    operation can be undone.
  What it means for the UI: >
    Show operations as history of the repository, each with its principal.
- jj concept: Conflict
  Closest git equivalent: Merge conflict
  Where the analogy breaks: >
    A conflict is stored in a commit and doesn't block work. It can be
    resolved later.
  What it means for the UI: >
    A conflicted change is a normal state. Mark it and list the conflicted
    files.
- jj concept: Divergent change
  Closest git equivalent: None
  Where the analogy breaks: >
    Two commits can share one change ID after concurrent rewrites.
  What it means for the UI: >
    Show every commit of a divergent change, and never pick one in silence.
- jj concept: Evolution
  Closest git equivalent: Reflog of one branch
  Where the analogy breaks: >
    The evolution log lists every earlier version of one change.
  What it means for the UI: A change page links to its earlier versions.
- jj concept: Revset
  Closest git equivalent: Revision range (`a..b`)
  Where the analogy breaks: >
    A revset is a small query language: `trunk()..@`, `author(alice)`,
    `conflicts()`.
  What it means for the UI: >
    Accept revisions in the address, and report an ambiguous one with its
    candidates.
- jj concept: Stack
  Closest git equivalent: A branch of dependent commits
  Where the analogy breaks: >
    A stack is a chain of changes. Rewriting one rebases the rest
    automatically.
  What it means for the UI: >
    Review and landing work on stacks, not on single branches.
- jj concept: Remote
  Closest git equivalent: A git remote
  Where the analogy breaks: >
    jjforge has no git. `jf` syncs with the forge in jj's own model, and
    plain `jj` and `git` can't reach it.
  What it means for the UI: Clone instructions name `jf` only.
```

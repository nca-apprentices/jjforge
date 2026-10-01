# Concepts

jjforge shows Jujutsu (jj) repositories. Most people arrive knowing git, so
this page maps each jj concept to its closest git equivalent, says where the
analogy breaks, and says what that means for the UI. The
[jj documentation](https://jj-vcs.github.io/jj/latest/git-comparison/) covers
the concepts in depth.

| jj concept           | Closest git equivalent        | Where the analogy breaks                                                                          | What it means for the UI                                                                     |
| -------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Change               | Commit                        | A change keeps its identity when it is rewritten. Each rewrite produces a new commit.             | The change is the unit the UI shows, links to, and reviews. There is no pull request object. |
| Change ID            | Commit hash                   | The change ID is stable across rewrites. The commit ID changes with every rewrite.                | Change IDs lead and link. Commit IDs are secondary detail.                                   |
| Working-copy commit  | Index and working tree        | jj records the working copy as a commit. There is no staging area.                                | The server has no working copy, so the UI never shows `@`.                                   |
| Bookmark             | Branch                        | A bookmark doesn't move when a new change is made on top of it. It moves only when set or pushed. | Call it a bookmark, never a branch. Show where each one points.                              |
| Trunk                | `main`                        | `trunk()` is a revset that names the main bookmark, not a special branch.                         | Trunk is the default revision and the landing target.                                        |
| Operation log        | Reflog                        | The operation log records every change to the whole repository, and any operation can be undone.  | Show operations as history of the repository, each with its principal.                       |
| Conflict             | Merge conflict                | A conflict is stored in a commit and doesn't block work. It can be resolved later.                | A conflicted change is a normal state. Mark it and list the conflicted files.                |
| Divergent change     | None                          | Two commits can share one change ID after concurrent rewrites.                                    | Show every commit of a divergent change, and never pick one in silence.                      |
| Evolution            | Reflog of one branch          | The evolution log lists every earlier version of one change.                                      | A change page links to its earlier versions.                                                 |
| Revset               | Revision range (`a..b`)       | A revset is a small query language: `trunk()..@`, `author(alice)`, `conflicts()`.                 | Accept revisions in the address, and report an ambiguous one with its candidates.            |
| Stack                | A branch of dependent commits | A stack is a chain of changes. Rewriting one rebases the rest automatically.                      | Review and landing work on stacks, not on single branches.                                   |
| Colocated repository | A git repository              | jj and git share one `.git` directory, so git tools keep working.                                 | Clone instructions work for both jj and git.                                                 |

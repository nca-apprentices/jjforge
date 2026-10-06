---
name: jf-commit
description: Commit in this repository with its message conventions. Use whenever you commit here.
---

# Commit

PRs are squash-merged with their commit messages, so a branch holds one
logical change, and its message lands on `main`.

1. **Kind.** `gh issue view <n> --json issueType,parent` names the issue's
   type and epic. The table gives the subject and the footer for each kind.
2. **Stage.** Run `git status` and `git diff`, then `git add` only the files
   of this change. Leave other untracked files alone.
3. **Draft.** Write the subject, the body, and the footer by the rules
   below.
4. **Confirm.** Show the message and wait for approval before
   `git commit`.

## Subject by kind

| Kind                   | Subject                                                                                        | `Refs:`                              |
| ---------------------- | ---------------------------------------------------------------------------------------------- | ------------------------------------ |
| Spec of an epic        | `spec: <what people can do>`                                                                   | `#<epic>`                            |
| Build of a requirement | `feat(<scope>): <what a person can do>`                                                        | `#<requirement>, #<epic>`            |
| Task                   | `feat`, `refactor`, `perf`, `test`, `docs`, `ci`, or `chore`, with a scope, by what it changes | `#<task>, #<epic>`                   |
| Bug                    | `fix(<scope>): <what works again>`                                                             | `#<bug>`, and its epic if it has one |

- The type and the description are lowercase and imperative, at most 72
  characters in all, with no final period.
- The scope is the part of the repository the change touches: `jf`, `vcs`,
  `rust`, `jvm`, `web`, `api`, `e2e`, `adr`, `deploy`, `ci`, or `agents`.
  `git log --format=%s` shows each in use.
- A change that breaks a contract, a task name, or a stored format adds `!`
  after the scope, as in `feat(api)!:`.

## Body

Wrapped at 72. It says why the change matters, since the diff shows what.
For a bug, it names what broke and the cause, and the commit holds the test
that failed before the fix.

## Footer

Each footer is a git trailer, one per line, after one blank line, in this
order. No other trailer appears.

1. `Breaking-Change: <what breaks, and how to migrate>`. Required with `!`,
   and only then.
2. `Refs: #<n>, #<m>`. Required. The issue the commit works on comes first,
   then its epic.
3. `Co-Authored-By: <Name> <email>`. Only for a person who wrote part of
   the change.

A commit never closes an issue. The PR body does, with `Closes #<n>`, where
the Spec workflow checks it.

```text
fix(vcs): refuse an upload whose hash doesn't match

vcs stored an object under the name the client sent, so a client could
shadow another object. vcs now hashes each object before it stores it,
and a test uploads a mismatched one.

Refs: #120, #29
```

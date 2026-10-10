# Agent instructions

[Development](shared/docs/development.md) says how to build and check jjforge.
[Workflow](shared/docs/workflow.md) says how work is planned and how a
requirement goes from spec to code, and the
[ADRs](shared/docs/adr/README.md) say why.

## Skills

Work that a skill covers goes through it.

| Work                          | Skill         |
| ----------------------------- | ------------- |
| Specify an epic               | `/jf-specify` |
| Build a requirement or a task | `/jf-impl`    |
| Review a pull request         | `/jf-review`  |
| Commit                        | `/jf-commit`  |
| Open a pull request           | `/jf-pr`      |

## Writing

`mise run prose` checks every Markdown file against the Google developer
documentation style, and CI fails on a warning. Write so it passes the first
time:

- Present tense, active voice, and one idea per sentence.
- The fewest words that say it. No "simply," "just," or "easy."
- No em dash, en dash, or semicolon.
- One word for one thing, the same word every time.
- A comment says what its block does and why, never what a name says.

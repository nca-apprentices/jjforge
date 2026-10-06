# Agent instructions

[Development](shared/docs/development.md) says how to build and check jjforge.
[Workflow](shared/docs/workflow.md) says how work is planned and how a
requirement goes from spec to code, and the
[ADRs](shared/docs/adr/README.md) say why.

## Writing

`mise run prose` checks every Markdown file against the Google developer
documentation style, and CI fails on a warning. Write so it passes the first
time:

- Present tense, active voice, and one idea per sentence.
- The fewest words that say it. No "simply," "just," or "easy."
- No em dash, en dash, or semicolon.
- One word for one thing, the same word every time.
- A comment says what its block does and why, never what a name says.

# Agent instructions

[Development](shared/docs/development.md) says how to build and check
jjforge, and [planning](shared/docs/planning.md) how work is planned.

## Workflows

Work that a skill or an agent covers goes through it. Say which one you use
before you start, and don't do its steps by hand.

| Work                               | Use                               |
| ---------------------------------- | --------------------------------- |
| Write the spec of a requirement    | `/jjforge-specify`                |
| Review a spec, or a spec PR        | the `jjforge-spec-reviewer` agent |
| Build a requirement once specified | `/jjforge-implement`              |

Other work follows planning and the [ADRs](shared/docs/adr/README.md). A
step done by hand twice is a candidate for a new skill.

## Writing

`mise run prose` checks every Markdown file against the Google developer
documentation style, and CI fails on a warning. Write so it passes the first
time:

- Present tense, active voice, and one idea per sentence.
- The fewest words that say it. No "simply," "just," or "easy."
- No em dash, en dash, or semicolon.
- One word for one thing, the same word every time.
- A comment says what its block does and why, never what a name says.

## Keeping instructions current

An instruction names a file, a task, or a skill only if it exists.
`mise run agents:lint`, part of `mise run lint`, checks the paths, the
`mise run` tasks, and the skill and agent names in this file and in
`.claude/`. A skill links to the ADR that decides a rule instead of
restating it, and changes in the same PR as that rule.

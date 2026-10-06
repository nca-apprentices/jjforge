---
name: jf-impl
description: Build one specified requirement or one task until its checks pass. Use when asked to implement, build, or code an issue such as #31.
---

# Build a requirement or a task

The build PR of [the workflow](../../../shared/docs/workflow.md#specifying-and-building).
The spec is the source of truth, and this PR makes it pass without changing
it.

1. **Gate.** `gh issue view <n> --json issueType`. A requirement needs a
   section on its epic's page in `shared/docs/specs/` on `main`, and
   pending scenarios. Without them, stop and run `/jf-specify` first. If
   `git log --format=%an` on the page names you, ask before building. A
   task skips this step and the next.
2. **Red.** Remove the `# pending` line from each of the requirement's
   scenarios. Start `mise run up` in the background, run `mise run e2e`, and
   watch them fail.
3. **Build.** Follow the Flow section step by step. Use the traits and calls
   the Libraries section names, and keep to the module rules of
   [ADR 0003](../../../shared/docs/adr/0003-checked-code-rules.md).
4. **Gaps.** A scenario or a step that can't be built as written is a gap in
   the spec. Stop and fix the spec in its own PR with `/jf-specify`.
5. **Green.** `mise run test`, `mise run lint`, and `mise run e2e` pass.
   `mise run compose logs` shows no error, and the state that the
   Persistence section names exists.
6. **Ship.** Commit with `/jf-commit` and open the PR with `/jf-pr`. The body
   lists what step 5 showed.

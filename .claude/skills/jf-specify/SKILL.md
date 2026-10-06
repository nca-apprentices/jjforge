---
name: jf-specify
description: Write or extend the spec PR of an epic, with its page in shared/docs/specs/, pending scenarios, and contract changes. Use when asked to specify or spec an epic or a requirement, or to write its scenarios.
---

# Specify an epic

The spec PR of [the workflow](../../../shared/docs/workflow.md#specifying-and-building).
The [specs README](../../../shared/docs/specs/README.md#format) defines each
section, and [29-push-and-clone.md](../../../shared/docs/specs/29-push-and-clone.md)
shows every section filled in.

1. **Read.** The epic and every requirement under it:
   `gh issue view <epic>`, then its sub-issues through
   `gh api graphql` on `subIssues`. Read the ADRs that decide the epic's paths.
   Done when you can name each requirement and each ADR.
2. **Page.** Copy [template.md](template.md) to
   `shared/docs/specs/<epic>-<slug>.md` and add its row to the specs README.
   Each requirement section says what a person can do and its limits, in
   words that name no endpoint, command, or technology.
3. **Design.** Fill Flow, Persistence, Architecture, and Failures. Every
   refusal sits at the step that makes it, with its problem code.
4. **Libraries.** For each library a path builds on, read its source at the
   pinned version, in the cargo registry or the Gradle cache, never from
   memory. Name the traits jjforge implements, map each type that crosses
   into `shared/proto/` field by field, list the calls jjforge makes, and
   list what it leaves unused. Done when every type in the store or the
   contract has a row.
5. **Scenarios.** One file per requirement, `<n>-<slug>.hurl` in
   `shared/e2e/http/` or `<n>-<slug>.bats` in `shared/e2e/cli/`, with
   `# pending` as the first line. Use concrete values, check the problem code
   of every refusal and the exit code that `shared/docs/cli.md` gives it. A
   Bats file loads `forge` for its helpers.
6. **Contract.** Change `shared/api/` and `shared/proto/` until every assert
   is expressible. Change the contract, never the scenario.
7. **Unknowns.** Each question the page can't answer yet goes under
   `## Open questions` with the issue that answers it.
8. **Check.** `mise run prose`, `mise run lint`, and `mise run api:breaking`
   pass.
9. **Ship.** Commit with `/jf-commit` and open the PR with `/jf-pr`, as a
   draft while the page has open questions.

Next, once the PR merges: `/jf-impl <n>` for each requirement, by someone
other than the spec's author.

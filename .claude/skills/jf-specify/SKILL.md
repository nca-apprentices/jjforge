---
name: jf-specify
description: Write or extend the spec PR of an epic, with its page in docs/specs/, pending scenarios, and contract changes. Use when asked to specify or spec an epic or a requirement, or to write its scenarios.
---

# Specify an epic

The spec PR of [the workflow](../../../docs/workflow.md#specifying-and-building).
The [specs README](../../../docs/specs/README.md#format) defines each
section. [29-push-and-clone.md](../../../docs/specs/29-push-and-clone.md)
shows the reference half filled in, and
[143-review-model.md](../../../docs/specs/143-review-model.md) the
design half.

1. **Read.** The epic and every requirement under it:
   `gh issue view <epic>`, then its sub-issues through
   `gh api graphql` on `subIssues`. Read the ADRs that decide the epic's paths.
   Done when you can name each requirement and each ADR.
2. **Page.** Copy [template.md](template.md) to
   `docs/specs/<epic>-<slug>.md` and add its row to the specs README.
   Keep the design half when the epic adds an object, a state, or a protocol
   that another epic builds on.
3. **Design half.** Write Model first: each object, its identity, its
   invariants, and its state diagram. Then Guide, as a session by the seed's
   people that touches every object. Then Alternatives, one table per choice,
   and Prior art, read from each system's own documentation, never from
   memory. A choice that shapes more than one component becomes an ADR from
   [the template](../../../docs/adr/0000-template.md) in the same PR.
   Write Summary and Motivation last. Done when a reader of the design half
   alone can say what is stored, what moves each state, and why.
4. **Requirements.** Each requirement section links its issue and derives
   its functional requirements, operations, and types from it, as the
   format in the README gives. Done when every functional requirement names
   a command or a route, what it does, and each refusal.
5. **Reference half.** Fill Flow, Persistence, Architecture, and Failures,
   with a sequence diagram per write path that crosses binaries. Every
   refusal sits at the step that makes it, with its problem code. Add
   Compatibility when a contract changes, and Operations when a path has a
   limit or a queue.
6. **Libraries.** For each library a path builds on, read its source at the
   pinned version, in the cargo registry or the Gradle cache, never from
   memory. Name the traits jjforge implements, map each type that crosses
   into `contracts/proto/` field by field, list the calls jjforge makes, and
   list what it leaves unused. Done when every type in the store or the
   contract has a row.
7. **Scenarios.** One file per requirement, `<n>-<slug>.hurl` in
   `e2e/http/` or `<n>-<slug>.bats` in `e2e/cli/`, with
   `# pending` as the first line. Each test name starts with the ID of the
   functional requirement it checks. Use concrete values, check the problem code
   of every refusal and the exit code that `docs/cli.md` gives it. A
   Bats file loads `forge` for its helpers.
8. **Contract.** Change `contracts/openapi/` and `contracts/proto/` until
   every assert is expressible. Change the contract, never the scenario. A
   REST operation goes in the `paths.yaml` of its aspect, and
   `contracts/openapi/openapi.yaml` references its path. Each parameter,
   header, and body carries an example that uses the values of the scenario.
9. **Unknowns.** Each question the page can't answer yet goes under
   `## Open questions` with the issue that answers it.
10. **Check.** `mise run prose`, `mise run lint`, and `mise run api:breaking`
    pass.
11. **Ship.** Commit with `/jf-commit` and open the PR with `/jf-pr`. A page
    with open questions merges, and its requirements wait for the answers.

Next, once the PR merges: `/jf-impl <n>` for each requirement, by someone
other than the spec's author.

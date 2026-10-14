# Specs

One page per epic says what the epic is for, how it works and why, and what
each of its requirements does.
[ADR 0004](../adr/0004-specified-tested-built.md) decides when a page is
written and what holds it to this form.

| Spec                                   | Epic                                                        |
| -------------------------------------- | ----------------------------------------------------------- |
| [Orgs and repos](23-orgs-and-repos.md) | [#23](https://github.com/nca-apprentices/jjforge/issues/23) |
| [Push and clone](29-push-and-clone.md) | [#29](https://github.com/nca-apprentices/jjforge/issues/29) |
| [Review model](143-review-model.md)    | [#143](https://github.com/nca-apprentices/jjforge/issues/143) |

## Format

A page has two halves. The design half says why the epic exists, how it
feels to use, what it stores, and what else was considered. The reference
half says what each requirement does and how each path runs. The design half
borrows from
[Rust RFCs](https://github.com/rust-lang/rfcs/blob/master/0000-template.md),
[Kubernetes KEPs](https://github.com/kubernetes/enhancements/tree/master/keps/NNNN-kep-template),
and [jj's design docs](https://github.com/jj-vcs/jj/blob/main/docs/design_doc_blueprint.md).

[Orgs and repos](23-orgs-and-repos.md) shows the required sections,
[Push and clone](29-push-and-clone.md) the optional reference sections, and
[Review model](143-review-model.md) the design half. In order:

1. The filename, `<epic number>-<slug>.md`, the epic's name as the title,
   and a first paragraph that links the epic, names the module that owns
   it, and links the ADRs its paths follow.
2. `## Summary`, design: what the epic adds, in at most six sentences, for
   a reader who reads nothing else.
3. `## Motivation`, design: the problem, who has it, the cost of leaving it,
   and the goals as outcomes a demo can show.
4. `## Guide`, design: one worked example, as if the epic were built. Real
   `jf` commands with their output, or requests with their answers, by the
   people of the seed. It teaches the concepts in the order a reader meets
   them.
5. `## Model`, design: the objects the epic adds, what identifies each, the
   invariants that always hold, and a Mermaid state diagram for each object
   that changes state. It says which facts are stored and which are derived
   from them.
6. One section per requirement, headed with its name and issue number, as in
   `## Create a repository (#25)`. The issue holds the statement, so the
   section links it and derives from it, in order:
   - A table of functional requirements, each with the ID `<n>.<k>`, and
     each one testable: a command or a route, what it does, and every
     refusal with its problem code.
   - The operations the requirement involves: routes of the contract and
     commands of `jf`.
   - The types it involves: messages of `shared/proto/`, schemas of
     `shared/api/`, and the items of the Architecture section and the
     Libraries section.
   - A link to its path in the Flow section.
   - A link to each of its scenarios, `<issue number>-<slug>` in
     `shared/e2e/`. A Bats test name, or the comment that precedes a Hurl request,
     starts with the ID it checks.
7. `## Out of scope`, optional: what a reader could expect from the epic but
   won't get, each with the epic or the decision that covers it instead.
8. `## Flow`: each write path as numbered steps from its operation, with
   each refusal at the step that makes it, and a Mermaid sequence diagram
   when the path crosses the server, vcs, and the store.
9. `## Persistence`: each object path, each table with its constraints and
   its queries, and each event with its listeners. A Mermaid entity
   diagram shows the tables when there are more than two.
10. `## Architecture`: the modules the epic touches and the external systems
    its paths call, each with its twin, as
    [ADR 0004](../adr/0004-specified-tested-built.md) decides.
11. `## Libraries`, optional: each library a path builds on beyond the
    framework, with its pinned version, the traits jjforge implements, the
    types and calls it uses, and what it leaves unused.
12. `## Compatibility`, optional: what happens while `jf`, the server, and
    vcs run different versions, and what each contract change adds.
13. `## Operations`, optional: the signals that show each path is healthy,
    its limits, and how it grows with load.
14. `## Security`, optional: what each path trusts, what it verifies, and the
    limits that bound a hostile client.
15. `## Failures`: what each partial failure leaves behind and who repairs it,
    in prose.
16. `## Tests`, optional: the tests besides the scenarios, such as
    conformance tests of a library's traits, and what each one proves.
17. `## Alternatives`, design: each choice the page makes, as a table of
    options with what each costs, and the drawbacks of the chosen design. A
    choice that shapes more than one component is an ADR, and the page
    links it instead.
18. `## Prior art`, optional: each system that solved the same problem,
    what jjforge takes from it, and what it leaves.
19. `## Future possibilities`, optional: extensions the design leaves room
    for, each with the epic that would build it.
20. `## Open questions`: each question with the issue that answers it. A
    page with open questions merges, and no requirement on it is built
    until the section is gone. The Spec workflow checks that.

The design half is required when an epic adds an object, a state, or a
protocol that another epic builds on. A page with `## Model` has the other
design sections too, and `mise run prose` checks that. A small epic leaves
the design half out.

A read-only epic keeps the four required reference sections and says in
each what the read paths need from it.

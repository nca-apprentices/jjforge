# Specs

One page per epic says what each of its requirements does and how the epic
works inside. [ADR 0004](../adr/0004-specified-tested-built.md) decides when a
page is written and what holds it to this form.

| Spec                                   | Epic                                                        |
| -------------------------------------- | ----------------------------------------------------------- |
| [Orgs and repos](23-orgs-and-repos.md) | [#23](https://github.com/nca-apprentices/jjforge/issues/23) |
| [Push and clone](29-push-and-clone.md) | [#29](https://github.com/nca-apprentices/jjforge/issues/29) |

## Format

[Orgs and repos](23-orgs-and-repos.md) shows the required sections, and
[Push and clone](29-push-and-clone.md) the optional ones. In order:

1. The filename, `<epic number>-<slug>.md`, the epic's name as the title,
   and a first paragraph that links the epic and
   names the module that owns it.
2. One section per requirement, headed with its name and issue number, as in
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
3. `## Out of scope`, optional: what a reader could expect from the epic but
   won't get, each with the epic or the decision that covers it instead.
4. `## Flow`: each write path as numbered steps from its operation, with
   each refusal at the step that makes it, and a Mermaid sequence diagram
   when the path crosses the server, vcs, and the store.
5. `## Persistence`: each object path, each table with its constraints and
   its queries, and each event with its listeners.
6. `## Architecture`: the modules the epic touches and the external systems
   its paths call, each with its twin, as
   [ADR 0004](../adr/0004-specified-tested-built.md) decides.
7. `## Libraries`, optional: each library a path builds on beyond the
   framework, with its pinned version, the traits jjforge implements, the
   types and calls it uses, and what it leaves unused.
8. `## Security`, optional: what each path trusts, what it verifies, and the
   limits that bound a hostile client.
9. `## Failures`: what each partial failure leaves behind and who repairs it,
   in prose.
10. `## Tests`, optional: the tests besides the scenarios, such as
    conformance tests of a library's traits, and what each one proves.
11. `## Open questions`, only in a draft: each question with the issue that
    answers it. The page merges without it.

A read-only epic keeps the four required sections and says in each what the
read paths need from it.

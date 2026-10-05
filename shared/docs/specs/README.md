# Specs

One page per epic says what each of its requirements does and how the epic
works inside. [ADR 0017](../adr/0017-specified-before-built.md) decides when a
page is written and what holds it to this form.

| Spec                                     | Epic                                                        |
| ---------------------------------------- | ----------------------------------------------------------- |
| [Orgs and repos](23-orgs-and-repos.md)   | [#23](https://github.com/nca-apprentices/jjforge/issues/23) |

## Format

[Orgs and repos](23-orgs-and-repos.md) shows the format. In order:

1. The filename, `<epic number>-<slug>.md`, the epic's name as the title,
   and a first paragraph that links the epic and
   names the module that owns it.
2. One section per requirement, headed with its name and issue number, as in
   `## Create a repository (#25)`. The body says what a person can do, each
   refusal with its problem code, the operations and commands it adds, and
   limits such as size or latency. It ends with a link to its scenario in
   `shared/e2e/`.
3. `## Flow`: each write path as numbered steps, with a Mermaid sequence
   diagram when the path crosses the server, vcs, and the store.
4. `## Persistence`: each stream and its events, each object path, and each
   read model with its queries, with a Mermaid flowchart from stream to
   projector to read model.
5. `## Architecture`: the modules the epic touches and the external systems
   its paths call, each with its twin, as
   [ADR 0018](../adr/0018-external-systems-run-as-twins.md) decides.
6. `## Failures`: what each partial failure leaves behind and who repairs it,
   in prose.

A read-only epic keeps the four sections and says in each what the read paths
need from it.

# Designs

A design says how a write path flows: the policy, the claimed names, the
stream and its events, the read model, and what a partial failure leaves
behind. One file per epic, linked from the epic's Design field, growing one
section per spec PR. A read path needs none. [Planning](../planning.md) has
the rules, and [ADR 0017](../adr/0017-specified-before-built.md) the
decision.

| Design                                      | Epic                                                        |
| ------------------------------------------- | ----------------------------------------------------------- |
| [E10 Orgs and repos](E10-orgs-and-repos.md) | [#23](https://github.com/nca-apprentices/jjforge/issues/23) |

## Format

- A section starts with `designs #25 #26`, which `mise run spec:trace`
  reads.
- Steps in order, each refusal with its problem code, then the partial
  failures and who repairs them, then the read model and its queries.
- The event schema is `shared/proto/<module>/v1/events.proto`, in the same
  PR.
- A Mermaid diagram where it says more than the list: a sequence diagram for
  a command that crosses the server, vcsd, and the store, a flowchart for
  data between a stream, a projector, and a read model. Failures stay in
  prose.

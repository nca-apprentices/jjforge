# Designs

A design says how a write path flows: the policy, the claimed names, the
stream and its events, the read model, and what a partial failure leaves
behind. [ADR 0017](../adr/0017-specified-before-built.md) decides when one
is written.

| Design                                      | Epic                                                        |
| ------------------------------------------- | ----------------------------------------------------------- |
| [E10 Orgs and repos](E10-orgs-and-repos.md) | [#23](https://github.com/nca-apprentices/jjforge/issues/23) |

## Format

- Steps in order, each refusal with its problem code, then the partial
  failures and who repairs them, then the read model and its queries.
- A Mermaid diagram where it says more than the list: a sequence diagram for
  a command that crosses the server, vcsd, and the store, a flowchart for
  data between a stream, a projector, and a read model. Failures stay in
  prose.

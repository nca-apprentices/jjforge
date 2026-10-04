# Designs

A design says how a write path flows through the system: which policy
decides, which name is claimed, which kernel stream the module appends to,
which events it writes, which read model a projector builds, and what each
partial failure leaves behind. A read path needs no design, because the
contract and its scenarios say everything about it.

There is one design per epic, named after it, and the epic's Design field
links to it. A design grows one section per spec PR, so a section exists only
once its requirements are specified. [Planning](../planning.md) has the
rules, and [ADR 0017](../adr/0017-specified-before-built.md) the decision.

| Design                                      | Epic                                                        |
| ------------------------------------------- | ----------------------------------------------------------- |
| [E10 Orgs and repos](E10-orgs-and-repos.md) | [#23](https://github.com/nca-apprentices/jjforge/issues/23) |

## Format

- The first line under a section heading cites the requirements the section
  serves, as `designs #25 #26 #15`. `mise run spec:trace` reads these lines.
- A section names its steps in order, each with the problem code it answers
  on refusal, then the partial failures and who repairs them, then the read
  model and its queries.
- The event schema is a proto file under `shared/proto/<module>/v1/`, in the
  same PR as the section. The projector's test lands with the implementation.
- A design links to the ADRs it depends on. An ADR never links to a design,
  because a decision outlives the requirements that prompted it.

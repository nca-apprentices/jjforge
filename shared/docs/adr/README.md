# Architecture decision records

Each record states one decision and why it was made.
[ADR 0001](0001-record-architecture-decisions.md) sets the rules: a record is
never edited, and a newer one supersedes it. New records start from
[the template](0000-template.md).

| ADR                                             | Decision                                                      | Status   |
| ----------------------------------------------- | ------------------------------------------------------------- | -------- |
| [0001](0001-record-architecture-decisions.md)   | Record architecture decisions                                 | Accepted |
| [0002](0002-public-api-is-http.md)              | The public API is HTTP only                                   | Accepted |
| [0003](0003-tenant-keys-and-pagination.md)      | The tenant leads every key                                    | Accepted |
| [0004](0004-identity-and-tokens.md)             | The forge issues its own tokens                               | Accepted |
| [0005](0005-modular-monolith.md)                | The server is a modular monolith                              | Accepted |
| [0006](0006-object-storage.md)                  | Objects live in SeaweedFS, and vcsd stays stateless           | Accepted |
| [0007](0007-git-bridge-first.md)                | The git bridge comes first                                    | Accepted |
| [0008](0008-postgres-on-cnpg.md)                | Postgres, not distributed SQL                                 | Accepted |
| [0009](0009-cli-on-jj-cli.md)                   | The CLI is `jf`, built on jj-cli                              | Accepted |
| [0010](0010-backing-services-in-infra.md)       | The infra repository runs the backing services                | Accepted |
| [0011](0011-controllers-implement-contracts.md) | Controllers implement contracts                               | Accepted |
| [0012](0012-web-packages.md)                    | The web app is split into apps, features, and shared packages | Accepted |
| [0013](0013-browser-state-in-libraries.md)      | Browser state belongs to a library                            | Accepted |
| [0014](0014-markup-and-styles-in-shared-ui.md)  | Only shared/ui renders markup and styles                      | Accepted |

# Architecture decision records

Each record states one decision and why it was made.
[ADR 0001](0001-record-architecture-decisions.md) sets the rules: until the
first release, a record is edited or deleted when its decision changes, and
after it a newer record supersedes it. New records start from
[the template](0000-template.md).

| ADR                                                 | Decision                                                      | Status   |
| --------------------------------------------------- | ------------------------------------------------------------- | -------- |
| [0001](0001-record-architecture-decisions.md)       | Record architecture decisions                                 | Accepted |
| [0002](0002-public-protocols.md)                    | The public protocols are REST and the sync protocol           | Accepted |
| [0003](0003-tenant-keys-and-pagination.md)          | The tenant leads every key                                    | Accepted |
| [0004](0004-identity-and-tokens.md)                 | The forge issues its own tokens                               | Accepted |
| [0005](0005-storage-kernel.md)                      | vcsd is the storage kernel, and the server owns the product   | Accepted |
| [0006](0006-object-store-is-the-source-of-truth.md) | The object store is the only source of truth                  | Accepted |
| [0007](0007-native-jj-without-git.md)               | jjforge stores and syncs jj data natively, without git        | Accepted |
| [0008](0008-postgres-holds-read-models.md)          | Postgres holds only read models                               | Accepted |
| [0009](0009-cli-on-jj-cli.md)                       | The CLI is `jf`, built on jj-cli                              | Accepted |
| [0010](0010-roles-and-deployment-modes.md)          | Both binaries run as roles, and the chart picks a mode        | Accepted |
| [0011](0011-controllers-implement-contracts.md)     | Controllers implement contracts                               | Accepted |
| [0012](0012-web-packages.md)                        | The web app is split into apps, features, and shared packages | Accepted |
| [0013](0013-browser-state-in-libraries.md)          | Browser state belongs to a library                            | Accepted |
| [0014](0014-markup-and-styles-in-shared-ui.md)      | Only shared/ui renders markup and styles                      | Accepted |
| [0015](0015-checked-code-rules.md)                  | Code rules are checked, not reviewed                          | Accepted |
| [0016](0016-contract-in-typespec.md)                | The REST contract is written in TypeSpec                      | Accepted |

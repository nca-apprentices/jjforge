# Architecture decision records

Each record states one decision and why it was made.
[ADR 0001](0001-record-architecture-decisions.md) sets the rules: until the
first release, a record is edited or deleted when its decision changes, and
after it a newer record supersedes it. New records start from
[the template](0000-template.md).

| ADR                                           | Decision                                                                  | Status   |
| --------------------------------------------- | ------------------------------------------------------------------------- | -------- |
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions                                             | Accepted |
| [0002](0002-native-jj-without-git.md)         | Native jj without git: the format, the sync protocol, and `jf`            | Accepted |
| [0003](0003-where-state-lives.md)             | Repositories live in the object store, and the server's state in Postgres | Accepted |
| [0004](0004-vcs-and-server.md)                | vcs owns repositories, and the server owns the product                    | Accepted |
| [0005](0005-identity-and-tokens.md)           | The forge issues its own tokens                                           | Accepted |
| [0006](0006-contract-in-typespec.md)          | The REST contract is TypeSpec, served by generated controllers            | Accepted |
| [0007](0007-checked-code-rules.md)            | Code rules are checked, not reviewed                                      | Accepted |
| [0008](0008-web-app.md)                       | The web app: packages, state, and markup                                  | Accepted |
| [0009](0009-specified-tested-built.md)        | A requirement is specified, tested against twins, then built              | Accepted |

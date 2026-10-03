# jjforge documentation

jjforge is a forge for [Jujutsu](https://github.com/jj-vcs/jj) repositories
that grows into a modular platform for building software. These documents say
why it exists, how it fits together, and how to work on it.

| Document                           | What it answers                                    |
| ---------------------------------- | -------------------------------------------------- |
| [Architecture](architecture.md)    | How the parts fit together                         |
| [Concepts](concepts.md)            | jj for git users, and what that means for the UI   |
| [Surfaces](surfaces.md)            | Where each capability appears on each interface    |
| [CLI conventions](cli.md)          | How `jf` behaves: output, prompts, and exit codes  |
| [Planning](planning.md)            | Initiatives, epics, requirements, and their rules  |
| [Roadmap](roadmap.md)              | The initiatives and their epics                    |
| [Decisions](adr/README.md)         | The architecture decision records                  |
| [Development](development.md)      | Building, running, checking, and releasing         |

The contracts live next to these documents:
[shared/api](../api/) for the REST API in TypeSpec, and [proto](../proto/) for
the sync protocol, the native object format, and the internal gRPC contracts
between the server and vcsd.

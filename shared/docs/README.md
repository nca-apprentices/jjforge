# jjforge documentation

jjforge is a forge for [Jujutsu](https://github.com/jj-vcs/jj) repositories
that grows into a modular platform for building software. Read the sections
in order.

## The system

What jjforge is and how it works.

1. [Concepts](concepts.md): jj for git users, and what that means for the UI.
2. [Architecture](architecture.md): how the parts fit together.
3. [Surfaces](surfaces.md): where each capability appears on each interface.
4. [CLI conventions](cli.md): how `jf` behaves.
5. [Designs](design/README.md): how each write path flows, one per epic.

The [decisions](adr/README.md) say why. Each page links the ones it rests on.
The contracts are [shared/api](../api/) and [shared/proto](../proto/).

## The work

[Planning](planning.md) says how work is planned and tracked.

## Development

[Development](development.md) says how to build, run, check, and release.

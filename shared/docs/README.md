# jjforge documentation

jjforge is a forge for [Jujutsu](https://github.com/jj-vcs/jj) repositories
that grows into a modular platform for building software. Today it is an
echo server: every component exists, builds, deploys, and passes one message
along. The operations of the first release answer 501 until they are built.

## Using jjforge

1. [Concepts](concepts.md): jj for git users, and what that means for the UI.
2. [The CLI](cli.md): the settings, output, and exit codes of `jf`.

## Building jjforge

1. [Architecture](architecture.md): the parts, the building blocks, and how
   a release is deployed.
2. [Workflow](workflow.md): how an issue becomes a spec, then code.
3. [Development](development.md): the toolchain and its commands.

## Reference

- [Specs](specs/README.md): one page per epic.
- [Decisions](adr/README.md): one record per decision.

The contracts are [shared/api](../api/) and [shared/proto](../proto/).

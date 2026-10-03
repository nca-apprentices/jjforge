# CLI conventions

`jf` is the jjforge command. It builds on jj, so every jj command works, and it
adds the forge commands: `auth`, `org`, `repo`, `clone`, `fetch`, `push`,
`import`, and `browse`. It talks to the forge through the REST API and the sync protocol
`sync/v1`, and nothing else. See [ADR 0009](adr/0009-cli-on-jj-cli.md) and
[ADR 0002](adr/0002-public-protocols.md).

The conventions follow [clig.dev](https://clig.dev).

## Repositories

- `jf clone` fetches the operations and views of a repository, and the working
  copy fetches files as it needs them. `jf fetch --full` fetches everything,
  for working offline.
- `jf push` uploads the missing objects, then publishes. When someone else
  published first, `jf` fetches, merges, and tries again, or reports the
  conflict.
- `jf import <path>` copies the history of an existing jj repository into a
  jjforge repository, whatever backend the source uses.
- `jf init --store s3://bucket/repo` keeps a repository in a bucket without a
  forge, and `jf clone --store` reads one from its storage. Only people trusted
  with the bucket can write to it.

## Output

- Data goes to stdout, and messages go to stderr.
- `--json` prints exactly the schema that [openapi.yaml](../openapi.yaml)
  defines for the same data, so a script reads one shape everywhere
  ([#53](https://github.com/nca-apprentices/jjforge/issues/53)).
- Color appears only when stdout is a terminal, and `NO_COLOR` turns it off.

## Input

- Without a terminal, `jf` never prompts. A command that needs an answer fails
  and names the missing input
  ([#56](https://github.com/nca-apprentices/jjforge/issues/56)).
- A setting comes from a flag first, then the environment, then the
  configuration file. `JJFORGE_ENDPOINT` names the forge to talk to.

## Exit codes

Each kind of failure has its own exit code, so a script never parses text
([#54](https://github.com/nca-apprentices/jjforge/issues/54)). The problem
codes come from the `application/problem+json` answers of the API.

| Exit code | Meaning            | Problem codes                                   |
| --------- | ------------------ | ----------------------------------------------- |
| 0         | Success            | None                                            |
| 1         | Other error        | Every code not listed below                     |
| 2         | Bad usage          | None, clap's default for a usage error          |
| 3         | Ambiguous revision | `rev_ambiguous`                                 |
| 4         | Not found          | `not_found`, `rev_not_found`, `path_not_found`  |
| 5         | Not authorized     | `unauthenticated`, `forbidden`                  |
| 6         | Conflict           | `name_taken`, `precondition_failed`             |

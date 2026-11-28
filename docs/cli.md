# The CLI

`jf` is the jjforge command, built on jj as
[ADR 0001](adr/0001-native-jj-without-git.md) decides. Its conventions follow
[clig.dev](https://clig.dev).

## Sign-in

- `jf login --org <org>` signs a person in through the organization's
  provider with the device flow of RFC 8628. It prints a code and an
  address, and opens the browser at the address when a terminal and a
  browser exist. It saves the session in `$XDG_CONFIG_HOME/jjforge/credentials.toml`,
  readable by its owner only, and prints who signed in
  ([#9](https://github.com/nca-apprentices/jjforge/issues/9)).
- Every command renews the session's access token in the background, so a
  person signs in once per session of 12 hours
  ([#161](https://github.com/nca-apprentices/jjforge/issues/161)).
- `jf logout` ends the session at the forge and deletes the saved one.
- `jf op verify` checks the forge's signature on every operation that the
  forge's head reaches ([#163](https://github.com/nca-apprentices/jjforge/issues/163)).

## Repositories

- `jf fetch --full` fetches every object of a repository, for working
  offline. [ADR 0001](adr/0001-native-jj-without-git.md) says what a clone fetches.
- When someone else published first, `jf push` fetches, merges, and tries
  again, or reports the conflict.

## Output

- Data goes to stdout, and messages go to stderr.
- `--json` prints exactly the schema that [the contract](../contracts/openapi/)
  defines for the same data, so a script reads one shape everywhere
  ([#53](https://github.com/nca-apprentices/jjforge/issues/53)).
- Color appears only when stdout is a terminal, and `NO_COLOR` turns it off.

## Input

- Without a terminal, `jf` never prompts. A command that needs an answer fails
  and names the missing input
  ([#56](https://github.com/nca-apprentices/jjforge/issues/56)).
- A setting comes from a flag first, then the environment, then the
  configuration file. `JJFORGE_ENDPOINT` names the forge to talk to.
- `JJFORGE_TOKEN` holds an access token that `jf` sends in place of the
  saved session. Scripts and tests set it, and `jf` never renews it.

## Exit codes

Each kind of failure has its own exit code, so a script never parses text
([#54](https://github.com/nca-apprentices/jjforge/issues/54)). The problem
codes come from the `application/problem+json` answers of the API. A
sign-in that the provider or the person refuses, and a session that ended,
exit with 5.

| Exit code | Meaning            | Problem codes                                   |
| --------- | ------------------ | ----------------------------------------------- |
| 0         | Success            | None                                            |
| 1         | Other error        | Every code not listed below                     |
| 2         | Bad usage          | None, clap's default for a usage error          |
| 3         | Ambiguous revision | `rev_ambiguous`                                 |
| 4         | Not found          | `not_found`, `rev_not_found`, `path_not_found`  |
| 5         | Not authorized     | `unauthenticated`, `forbidden`                  |
| 6         | Conflict           | `name_taken`, `precondition_failed`             |

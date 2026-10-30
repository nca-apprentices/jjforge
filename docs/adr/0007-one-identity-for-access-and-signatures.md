# 0007. One identity for access and signatures

Status: accepted, 2026-10-30. Deciders: jjforge maintainers.

## Context

A git forge knows two identities that nothing ties together. A push is
authenticated by an SSH key or a token. The `author` and `committer` of a
commit are text that the client writes, so anyone can push a commit in another
person's name. A signature is the only link, and it needs a second key that
each person manages and uploads. The forge keeps who pushed in its own
database, never in the repository.

jjforge already closes half of this. vcs sets each operation's principal
from the request's token, and whoever writes an operation signs it, as
[ADR 0001](0001-native-jj-without-git.md) decides. The committer of a commit
is still text from the clone's `user.name` and `user.email`. A signed
operation proves only what vcs saw, so a compromised vcs or bucket, or a
direct repository, leaves a commit without proof of its writer.

[ADR 0002](0002-state-boundaries-and-tokens.md) authenticates a request with
a JWT. The [review model](../specs/143-review-model.md) signs each record
with SSHSIG, and needs a key that belongs to the same principal.

```yaml table
- Option: Committer as text, as git keeps it
  Cost: A commit names whoever its writer typed.
- Option: Signing keys that each person uploads, as GitHub takes them
  Cost: >
    A second identity, keys that live for years, and no narrower key for an
    agent.
- Option: Keyless signing through Sigstore's Fulcio and Rekor
  Cost: >
    Two services outside jjforge, and a signature that plain jj can't verify.
- Option: >
    The committer checked against the token, and short-lived keys that the
    server certifies
  Cost: >
    Every new commit needs a session, and the server holds a certificate
    authority. Chosen.
```

## Decision

### The committer is the principal

- `jf` writes the committer of each commit from the signed-in principal,
  never from `user.name` and `user.email`.
- vcs refuses a publish that makes a commit visible for the first time
  when its committer isn't the request's principal. A commit that
  the repository already holds passes, whoever pushes it again.
- The author stays free, because a person who rebases or amends another
  person's change keeps that person as author.
- The commits that the landing queue writes name vcs as committer, and
  their operation names the principal who asked to land.
- This rule needs no keys, so it holds before sign-in exists. Until then the
  principal is the name that `VCS_PRINCIPALS` lists.

### Keys are certified by the server

- `jf login` generates an Ed25519 key, and the server answers it with an SSH
  certificate next to the token. The certificate names the principal and the
  delegation chain, and it expires with the token.
- An agent's certificate expires with its narrower token, so a key never
  outlives the access it signs for.
- The server publishes its certificate authority's keys next to the keys
  that verify its tokens.
- The key stays on the machine that generated it. jjforge stores no private
  key of a person.

### Everything that names a principal is signed

- `jf` signs each commit in `secure_sig` with jj's SSH backend, under jj's
  namespace `git`.
- `jf` signs each review record under the namespace `jjforge-review`.
- vcs signs the operations and the landed commits it writes with its own
  certified key.
- The token authenticates the request, and the certificate authenticates
  the signature. vcs refuses a new commit or a record whose signature isn't
  valid, or whose certificate names another principal than the token.
- The server signs for a person of the web app with its own key and names
  the person in the delegation chain.

### A reader verifies

- A commit is verified when its signature is valid and its certificate
  names its committer.
- A certificate expires within hours, so a reader checks it at the time of
  the operation that first published the commit. vcs's clock sets that
  time, and vcs signs the operation.
- Plain `ssh-keygen -Y verify` and jj's own signature check verify a commit
  against an allowed signers file that lists the authority as
  `cert-authority`.

## Consequences

- Sign-in comes before signing. Until
  [#6](https://github.com/nca-apprentices/jjforge/issues/6) exists, vcs
  checks the committer and nothing is signed.
- `jf import` makes the importer the committer of every imported commit and
  signs it. The author and the change ID stay, and the old committer is
  lost.
- A direct repository has no vcs to check the committer. Its readers trust
  signatures only, as the review model already states.
- A leaked key of the certificate authority lets its holder sign as anyone
  until the server rotates it. A rotation leaves old signatures verifiable,
  because readers keep the old authority keys.
- A commit signed outside `jf`, with a key the server never certified,
  shows as unverified.

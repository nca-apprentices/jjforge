# 0008. Sign-in through a provider, sessions from the forge

Status: proposed, 2026-11-27. Deciders: jjforge maintainers.

## Context

[ADR 0002](0002-state-boundaries-and-tokens.md) decides that the server
issues every token as a JWT, that vcs verifies tokens offline, and that
revocation takes effect when a token expires. It leaves open who proves a
person's identity, what a token holds, and how long each credential lives.
[#6](https://github.com/nca-apprentices/jjforge/issues/6) needs those answers
in the server, vcs, `jf`, and the web app at once.

The people who use jjforge first already have GitHub accounts. GitHub signs
people in with OAuth 2.0 and a call to its REST API. It issues no ID token
for a person, because its OpenID Connect provider serves only GitHub Actions
workloads.

```yaml table
- Option: The provider's tokens as the forge's tokens
  Cost: >
    vcs trusts every provider, a token carries no forge scope, and revoking
    a session needs the provider.
- Option: The forge as its own authorization server, the provider for identity only
  Cost: The server implements the OAuth endpoints. Chosen.
- Option: OpenID Connect providers only
  Cost: GitHub can't sign anyone in.
- Option: GitHub only, through OAuth and its user API
  Cost: >
    A second kind of provider needs its own adapter. Chosen as the first
    kind.
```

A session must survive a working day, and a revoked one must stop soon.
vcs verifies a token without calling the server, so only a short expiry
bounds a token that vcs accepts after its session ends.

```yaml table
- Option: Long-lived access tokens, as personal access tokens work
  Cost: A stolen token works for weeks, and vcs can't see a revocation.
- Option: A revocation list that vcs fetches
  Cost: >
    One more feed to keep, and a list that vcs misses fails open or fails
    closed.
- Option: Short access tokens, renewed by a rotating refresh token
  Cost: >
    `jf` and the web app renew in the background, and two clients that share
    one refresh token race. Chosen.
```

## Decision

### The provider proves who a person is

- An organization connects one provider. Its kind is `github` first. An
  OpenID Connect kind follows behind the same interface.
- The server signs a person in with the authorization code flow and PKCE
  against the provider, reads who the person is, and discards the
  provider's token at once. jjforge stores no password and no provider
  token.
- An identity is the pair of the provider kind and the provider's stable
  subject: the numeric `id` of a GitHub user, never the `login`, because a
  GitHub login can change and pass to someone else. Each identity belongs to
  one principal.

### The forge issues its own credentials

The server is the authorization server of every jjforge client. The web app
and `jf` are its clients, with fixed client IDs, and neither ever talks to
the provider.

```yaml table
- Credential: Authorization code
  Lives: 60 seconds, used once
  Held by: The client, between the redirect and the token request
- Credential: Device code
  Lives: 15 minutes
  Held by: "`jf`, while the person confirms on another device"
- Credential: Access token
  Lives: 15 minutes
  Held by: The client, sent on every request to the server and to vcs
- Credential: Refresh token
  Lives: Until its session ends, replaced on each use
  Held by: The client, sent only to the token endpoint
- Credential: Session
  Lives: 12 hours from sign-in
  Held by: The server, as a row in Postgres
```

- An access token is a JWT signed with Ed25519. Its claims are `iss`, `aud`
  set to `jjforge`, `sub` set to the principal's ID, `name`, `sid` set to
  the session's ID, `scope`, `iat`, `nbf`, and `exp`. `exp` never passes the
  end of its session.
- The server verifies its own tokens and also reads the session row, so a
  revoked session stops at the server at once.
- vcs verifies the signature, `iss`, `aud`, `nbf`, and `exp` against the
  keys at `/oauth/keys`, which it fetches in the background. It calls the
  server for no request, so a revoked session stops at vcs when its last
  access token expires, within 15 minutes.
- A refresh token is used once. The server answers each refresh with a new
  one. A refresh token that was already used ends its session, because
  only a copy can present it twice.
- `jf login` uses the device flow of RFC 8628 on every machine, and opens
  the browser at the confirmation page when a terminal and a browser exist.
  `jf` takes a lock on its credentials file while it refreshes.

### vcs signs what it writes

vcs signs each operation it writes with an Ed25519 key of its own, under the
SSHSIG namespace `jjforge-operation`, and publishes the public key at
`/sync/v1/keys`. [ADR 0007](0007-one-identity-for-access-and-signatures.md)
certifies that key once the server's certificate authority exists. Until
then a reader trusts the key that the forge publishes.

## Consequences

- A session lost from Postgres signs its person out. A person signs in
  again, which costs a few seconds.
- A leaked signing key lets its holder mint tokens that vcs accepts until
  vcs fetches the keys again, at most 10 minutes after the key leaves
  `/oauth/keys`.
- An organization that wants sign-in through Okta or Entra ID waits for the
  OpenID Connect kind.
- The lifetimes are configuration, so compose shortens the access token to
  60 seconds and a scenario can watch one expire.

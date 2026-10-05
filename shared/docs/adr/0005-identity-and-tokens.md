# 0005. The forge issues its own tokens

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

People already have accounts at their organization's identity provider.
Agents and jobs later act on their behalf and need narrower, short-lived
access. vcs serves reads and must keep working when the sign-in service is
down ([#12](https://github.com/nca-apprentices/jjforge/issues/12)), and it
authorizes a push from the token alone, as
[ADR 0002](0002-native-jj-without-git.md) decides.

Options: pass the identity provider's tokens through, or run jjforge as its
own OAuth 2.0 authorization server. Upstream tokens carry no jjforge scopes,
so vcs would have to ask the server on every push, and `jf` would need a
client registered at every provider. Tokens can be JWTs or Biscuits. A
Biscuit can be narrowed offline by its holder, which no requirement needs
yet, and no Spring library issues one. Policy can live in code, OPA, or
Cedar.

## Decision

- Sign-in federates to an OpenID Connect provider that each organization
  connects. jjforge stores no passwords.
- jjforge is its own OAuth 2.0 authorization server, built on Spring
  Authorization Server. It offers the authorization code flow with PKCE for
  browsers and the device flow for machines without a browser.
- Access tokens are JWTs with jjforge scopes, signed by the server. A token
  that a sub-agent can narrow offline waits for a requirement that needs
  one, and replaces the format without replacing the flows.
- Cedar holds the authorization policy, evaluated in the server. Every
  decision is recorded in Postgres with the policy that made it.
- vcs verifies tokens offline against the published key set. Revocation takes
  effect when the current token expires, so tokens are short-lived.

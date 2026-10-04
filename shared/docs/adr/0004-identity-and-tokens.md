# 0004. The forge issues its own tokens

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

People already have accounts at their organization's identity provider.
Agents and jobs later act on their behalf and need narrower, short-lived
access. vcs serves reads and must keep working when the sign-in service is
down ([#12](https://github.com/nca-apprentices/jjforge/issues/12)).

Options: pass the identity provider's tokens through, or run jjforge as its
own OAuth 2.0 authorization server. Upstream tokens carry no jjforge scopes and
can't be narrowed offline. Tokens can be JWTs or Biscuits. Policy can live in
code, OPA, or Cedar.

## Decision

- Sign-in federates to an OpenID Connect provider that each organization
  connects. jjforge stores no passwords.
- jjforge is its own OAuth 2.0 authorization server. It offers the authorization
  code flow with PKCE for browsers and the device flow for machines without a
  browser.
- Access tokens are Biscuits. A holder can narrow a Biscuit offline, so a job
  can mint a token for a sub-agent without a round trip.
- Cedar holds the authorization policy, evaluated in the server. Every
  decision is recorded with the policy that made it.
- vcs verifies tokens offline against the published key set. Revocation takes
  effect when the current token expires, so tokens are short-lived.

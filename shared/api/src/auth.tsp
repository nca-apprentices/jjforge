import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

namespace jjforge;

/** A redirect to the identity provider. */
model Redirect {
  @statusCode status: 302;
  @header("Location") location?: url;
}

@tag("auth")
@summary("Describe the authorization server (RFC 8414)")
@returnsDoc("The server's endpoints and capabilities.")
@useAuth(NoAuth)
@get
@route("/.well-known/oauth-authorization-server")
op getAuthorizationServerMetadata(): AuthorizationServerMetadata | NotFound;

/** Redirects to the organization's identity provider, then back to `redirect_uri` with a code. PKCE with S256 is required. */
@tag("auth")
@summary("Start a browser sign-in")
@useAuth(NoAuth)
@get
@route("/oauth/authorize")
op authorize(
  @query response_type: "code",
  @query client_id: string,
  @query redirect_uri: url,
  @query scope?: string,
  @query state: string,
  @query code_challenge: string,
  @query code_challenge_method: "S256",
): Redirect | Invalid;

@tag("auth")
@summary("Start a sign-in on a machine without a browser (RFC 8628)")
@returnsDoc("The code to confirm on another device.")
@useAuth(NoAuth)
@post
@route("/oauth/device")
op authorizeDevice(
  @header contentType: "application/x-www-form-urlencoded",
  @body body: DeviceAuthorizationRequest,
): DeviceAuthorization | OAuthFailure;

/**
 * Grants: `authorization_code` after a browser sign-in, the device code
 * grant after a confirmed device sign-in, and `refresh_token`. The scope
 * `read` gives a read-only session, and `write` adds changes.
 */
@tag("auth")
@summary("Exchange a grant for a token")
@returnsDoc("A JWT access token.")
@useAuth(NoAuth)
@post
@route("/oauth/token")
op token(
  @header contentType: "application/x-www-form-urlencoded",
  @body body: TokenRequest,
): Token | OAuthFailure;

@tag("auth")
@summary("Revoke a token and its session (RFC 7009)")
@returnsDoc("Revoked, or already unknown.")
@post
@route("/oauth/revoke")
op revoke(
  @header contentType: "application/x-www-form-urlencoded",
  @body body: RevocationRequest,
): OkResponse | OAuthFailure;

/** vcs and other verifiers fetch these once and verify tokens offline. */
@tag("auth")
@summary("List the public keys that verify tokens")
@returnsDoc("The current and the next key.")
@useAuth(NoAuth)
@get
@route("/oauth/keys")
op getKeys(): KeySet | NotFound;

@tag("auth")
@summary("Show the signed-in principal")
@returnsDoc("The principal behind the token.")
@get
@route("/api/v1/me")
op getMe(): Principal | Unauthenticated;

@tag("auth")
@summary("List the signed-in principal's sessions")
@returnsDoc("One page of sessions.")
@get
@route("/api/v1/me/sessions")
op listSessions(...Parameters.Paging): SessionPage | Unauthenticated;

@tag("auth")
@summary("Revoke one session")
@returnsDoc("Revoked.")
@delete
@route("/api/v1/me/sessions/{session}")
op revokeSession(@format("uuid") @path session: string):
  | NoContentResponse
  | Unauthenticated
  | NotFound;

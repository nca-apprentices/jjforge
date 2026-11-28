# pending
# An owner connects acme to GitHub with the client ID and secret of the
# OAuth app that acme registered there, and acme's people sign in through
# it. The values are the seed's, so the scenario leaves acme as it found it.

# 7.1 An owner connects the provider. The answer never holds the secret.
PUT {{server}}/api/v1/orgs/acme/identity-provider
Authorization: Bearer {{acme_owner}}
{
  "issuer": "https://github.com",
  "clientId": "acme-jjforge",
  "clientSecret": "acme-twin-secret"
}
HTTP 200
[Asserts]
jsonpath "$.issuer" == "https://github.com"
jsonpath "$.clientId" == "acme-jjforge"
jsonpath "$.clientSecret" not exists

# 7.2 A sign-in through acme now goes to that provider with that client ID.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: connect-{{run}}
org: acme
code_challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
code_challenge_method: S256
HTTP 302
[Asserts]
header "Location" contains "/github/authorize?"
header "Location" contains "client_id=acme-jjforge"
header "Location" contains "%2Foauth%2Fcallback"

# 7.3 Only an owner connects one: a member gets 403.
PUT {{server}}/api/v1/orgs/acme/identity-provider
Authorization: Bearer {{acme_member}}
{
  "issuer": "https://github.com",
  "clientId": "member-app",
  "clientSecret": "member-secret"
}
HTTP 403
[Asserts]
jsonpath "$.code" == "forbidden"

# 7.4 Someone outside acme learns nothing about it: 404, never 403.
PUT {{server}}/api/v1/orgs/acme/identity-provider
Authorization: Bearer {{other_owner}}
{
  "issuer": "https://github.com",
  "clientId": "other-app",
  "clientSecret": "other-secret"
}
HTTP 404
[Asserts]
jsonpath "$.code" == "not_found"

# 7.5 GitHub is the only provider so far, so another issuer gets 400.
PUT {{server}}/api/v1/orgs/acme/identity-provider
Authorization: Bearer {{acme_owner}}
{
  "issuer": "https://login.acme.example",
  "clientId": "acme-jjforge",
  "clientSecret": "acme-twin-secret"
}
HTTP 400
[Asserts]
jsonpath "$.code" == "invalid"

# 7.6 A connection without a secret gets 400.
PUT {{server}}/api/v1/orgs/acme/identity-provider
Authorization: Bearer {{acme_owner}}
{
  "issuer": "https://github.com",
  "clientId": "acme-jjforge"
}
HTTP 400
[Asserts]
jsonpath "$.code" == "invalid"

# 7.7 A request without a token gets 401.
PUT {{server}}/api/v1/orgs/acme/identity-provider
{
  "issuer": "https://github.com",
  "clientId": "acme-jjforge",
  "clientSecret": "acme-twin-secret"
}
HTTP 401
[Asserts]
jsonpath "$.code" == "unauthenticated"

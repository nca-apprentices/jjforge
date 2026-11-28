# pending
# A person signs in with the GitHub account that acme connected, in the way
# a browser does: jjforge sends the browser to the provider, the provider
# sends it back, and the client trades a code for jjforge's own tokens. The
# twin's login form stands in for GitHub's. The verifier and its challenge
# are the example of RFC 7636.

# 8.1 jjforge sends the browser to acme's provider, with acme's client ID
# and a PKCE challenge of its own, never the client's.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: browser-{{run}}
org: acme
code_challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
code_challenge_method: S256
HTTP 302
[Captures]
provider: header "Location"
[Asserts]
header "Location" contains "/github/authorize?"
header "Location" contains "client_id=acme-jjforge"
header "Location" contains "code_challenge_method=S256"
header "Location" not contains "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
header "Location" not contains "browser-{{run}}"

# 8.2 The person signs in at the provider, which sends the browser back to
# jjforge's callback.
POST {{provider}}
[Form]
username: acme-owner
HTTP 302
[Captures]
callback: header "Location"
[Asserts]
header "Location" startsWith "{{server}}/oauth/callback?"

# 8.3 The callback reads who signed in and sends the browser to the client,
# with a jjforge code and the client's own state.
GET {{callback}}
HTTP 302
[Captures]
code: header "Location" regex "[?&]code=([^&]+)"
[Asserts]
header "Location" startsWith "{{server}}/callback?"
header "Location" contains "state=browser-{{run}}"

# 8.4 The client trades the code and its verifier for tokens.
POST {{server}}/oauth/token
[Form]
grant_type: authorization_code
client_id: web
code: {{code}}
redirect_uri: {{server}}/callback
code_verifier: dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
HTTP 200
[Captures]
access: jsonpath "$.access_token"
[Asserts]
jsonpath "$.token_type" == "Bearer"
jsonpath "$.expires_in" > 0
jsonpath "$.expires_in" <= 900
jsonpath "$.refresh_token" startsWith "jfr_"
jsonpath "$.scope" == "read write"

# 8.5 A code works once.
POST {{server}}/oauth/token
[Form]
grant_type: authorization_code
client_id: web
code: {{code}}
redirect_uri: {{server}}/callback
code_verifier: dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
HTTP 400
[Asserts]
jsonpath "$.error" == "invalid_grant"

# 8.6 The token names the person.
GET {{server}}/api/v1/me
Authorization: Bearer {{access}}
HTTP 200
[Asserts]
jsonpath "$.name" == "acme-owner"
jsonpath "$.kind" == "human"
jsonpath "$.id" matches "^[0-9a-f-]{36}$"

# 8.7 A code with the wrong verifier gets no token: a code alone, stolen
# from the redirect, is worth nothing.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: verifier-{{run}}
org: acme
code_challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
code_challenge_method: S256
HTTP 302
[Captures]
provider: header "Location"

POST {{provider}}
[Form]
username: acme-owner
HTTP 302
[Captures]
callback: header "Location"

GET {{callback}}
HTTP 302
[Captures]
code: header "Location" regex "[?&]code=([^&]+)"

POST {{server}}/oauth/token
[Form]
grant_type: authorization_code
client_id: web
code: {{code}}
redirect_uri: {{server}}/callback
code_verifier: wrong-verifier-wrong-verifier-wrong-verifier-0
HTTP 400
[Asserts]
jsonpath "$.error" == "invalid_grant"

# 8.8 A redirect address that the client didn't register gets a problem,
# never a redirect, so nobody can send a code to their own site.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: https://attacker.example/callback
state: attacker-{{run}}
org: acme
code_challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
code_challenge_method: S256
HTTP 400
[Asserts]
header "Location" not exists
jsonpath "$.code" == "invalid"

# 8.9 A request without an organization goes back to the client with
# `invalid_request`.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: no-org-{{run}}
code_challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
code_challenge_method: S256
HTTP 302
[Asserts]
header "Location" startsWith "{{server}}/callback?"
header "Location" contains "error=invalid_request"
header "Location" contains "state=no-org-{{run}}"

# 8.10 When the person declines at the provider, the client hears
# `access_denied`.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: denied-{{run}}
org: acme
code_challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
code_challenge_method: S256
HTTP 302
[Captures]
upstream_state: header "Location" regex "[?&]state=([^&]+)"

GET {{server}}/oauth/callback
[Query]
state: {{upstream_state}}
error: access_denied
HTTP 302
[Asserts]
header "Location" startsWith "{{server}}/callback?"
header "Location" contains "error=access_denied"
header "Location" contains "state=denied-{{run}}"

# 8.11 A callback whose state jjforge never issued gets a problem.
GET {{server}}/oauth/callback
[Query]
state: forged-{{run}}
code: 5c0a2f8e1b7d4e9a
HTTP 400
[Asserts]
jsonpath "$.code" == "invalid"

# 8.12 jjforge knows a person by GitHub's numeric ID, never by the login. A
# person who renamed their GitHub account stays the same principal.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: renamed-{{run}}
org: acme
code_challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
code_challenge_method: S256
HTTP 302
[Captures]
provider: header "Location"

POST {{provider}}
[Form]
username: acme-member-renamed
HTTP 302
[Captures]
callback: header "Location"

GET {{callback}}
HTTP 302
[Captures]
code: header "Location" regex "[?&]code=([^&]+)"

POST {{server}}/oauth/token
[Form]
grant_type: authorization_code
client_id: web
code: {{code}}
redirect_uri: {{server}}/callback
code_verifier: dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
HTTP 200
[Captures]
access: jsonpath "$.access_token"

GET {{server}}/api/v1/me
Authorization: Bearer {{access}}
HTTP 200
[Asserts]
jsonpath "$.name" == "acme-member"

# 8.13 A new person whose GitHub login another principal holds gets a new
# principal with the first free name.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: squatter-{{run}}
org: acme
code_challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
code_challenge_method: S256
HTTP 302
[Captures]
provider: header "Location"

POST {{provider}}
[Form]
username: squatter
HTTP 302
[Captures]
callback: header "Location"

GET {{callback}}
HTTP 302
[Captures]
code: header "Location" regex "[?&]code=([^&]+)"

POST {{server}}/oauth/token
[Form]
grant_type: authorization_code
client_id: web
code: {{code}}
redirect_uri: {{server}}/callback
code_verifier: dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
HTTP 200
[Captures]
access: jsonpath "$.access_token"

GET {{server}}/api/v1/me
Authorization: Bearer {{access}}
HTTP 200
[Asserts]
jsonpath "$.name" == "acme-owner-2"

# 8.14 A request without a token gets 401.
GET {{server}}/api/v1/me
HTTP 401
[Asserts]
jsonpath "$.code" == "unauthenticated"

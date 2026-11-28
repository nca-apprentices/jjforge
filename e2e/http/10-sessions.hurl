# pending
# A person signs in twice, as on a laptop and on a second machine, sees both
# sessions, and ends the second one from the first. Each sign-in is the
# browser flow of 8-browser-sign-in.hurl.

# Session A.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: a-{{run}}
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
code_verifier: dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
HTTP 200
[Captures]
access_a: jsonpath "$.access_token"

# Session B.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: b-{{run}}
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
code_verifier: dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
HTTP 200
[Captures]
access_b: jsonpath "$.access_token"

# 10.1 The list marks the session of the request's token as current, newest
# first, so session B leads.
GET {{server}}/api/v1/me/sessions
Authorization: Bearer {{access_b}}
HTTP 200
[Captures]
session_b: jsonpath "$.items[0].id"
[Asserts]
jsonpath "$.items[0].current" == true
jsonpath "$.items[0].client" == "web"
jsonpath "$.items[0].org" == "acme"
jsonpath "$.items[0].createdAt" isIsoDate
jsonpath "$.items[0].lastUsedAt" isIsoDate
jsonpath "$.items[0].expiresAt" isIsoDate

GET {{server}}/api/v1/me/sessions
Authorization: Bearer {{access_a}}
HTTP 200
[Captures]
session_a: jsonpath "$.items[?(@.current == true)].id" nth 0
[Asserts]
jsonpath "$.items[*].id" includes "{{session_b}}"
jsonpath "$.items[?(@.id == '{{session_b}}')].current" nth 0 == false

# 10.2 From session A, the person ends session B.
DELETE {{server}}/api/v1/me/sessions/{{session_b}}
Authorization: Bearer {{access_a}}
HTTP 204

# 10.3 An ended session leaves the list.
GET {{server}}/api/v1/me/sessions
Authorization: Bearer {{access_a}}
HTTP 200
[Asserts]
jsonpath "$.items[*].id" not includes "{{session_b}}"
jsonpath "$.items[*].id" includes "{{session_a}}"

# 10.4 Ending it again finds nothing.
DELETE {{server}}/api/v1/me/sessions/{{session_b}}
Authorization: Bearer {{access_a}}
HTTP 404
[Asserts]
jsonpath "$.code" == "not_found"

# 10.5 Nobody ends another person's session, nor learns that it exists.
DELETE {{server}}/api/v1/me/sessions/{{session_a}}
Authorization: Bearer {{other_owner}}
HTTP 404
[Asserts]
jsonpath "$.code" == "not_found"

GET {{server}}/api/v1/me/sessions
Authorization: Bearer {{other_owner}}
HTTP 200
[Asserts]
jsonpath "$.items[*].id" not includes "{{session_a}}"

GET {{server}}/api/v1/me/sessions
Authorization: Bearer {{access_a}}
HTTP 200
[Asserts]
jsonpath "$.items[*].id" includes "{{session_a}}"

# 10.6 A request without a token gets 401.
GET {{server}}/api/v1/me/sessions
HTTP 401
[Asserts]
jsonpath "$.code" == "unauthenticated"

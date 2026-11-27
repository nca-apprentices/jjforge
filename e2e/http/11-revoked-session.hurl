# pending
# An ended session stops working. The server checks the session on every
# request, so it refuses at once. vcs checks only the token's signature and
# expiry, so it refuses once the last access token expires. Compose gives an
# access token 60 seconds, and vcs allows 5 seconds of clock skew.
#
# The repository in the vcs requests doesn't exist: vcs checks the token
# before it looks, so 404 means the token passed and 401 means it didn't.

GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: revoke-{{run}}
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
refresh_1: jsonpath "$.refresh_token"

# 11.1 A refresh answers a new refresh token, and a new access token.
POST {{server}}/oauth/token
[Form]
grant_type: refresh_token
client_id: web
refresh_token: {{refresh_1}}
HTTP 200
[Captures]
access: jsonpath "$.access_token"
refresh_2: jsonpath "$.refresh_token"
[Asserts]
jsonpath "$.refresh_token" != "{{refresh_1}}"

GET {{sync}}/sync/v1/0199c9a0-0000-7000-8000-00000000ac3e/0199c9a0-0000-7000-8000-00000000dead/head
Authorization: Bearer {{access}}
HTTP 404

# 11.2 The person signs out: the client revokes its refresh token, as
# RFC 7009 decides, which ends the session.
POST {{server}}/oauth/revoke
[Form]
token: {{refresh_2}}
token_type_hint: refresh_token
HTTP 200

# 11.3 The server refuses the session's access token at once.
GET {{server}}/api/v1/me
Authorization: Bearer {{access}}
HTTP 401
[Asserts]
jsonpath "$.code" == "unauthenticated"

# 11.4 Its refresh token gets no new tokens.
POST {{server}}/oauth/token
[Form]
grant_type: refresh_token
client_id: web
refresh_token: {{refresh_2}}
HTTP 400
[Asserts]
jsonpath "$.error" == "invalid_grant"

# 11.5 vcs still accepts the access token until it expires.
GET {{sync}}/sync/v1/0199c9a0-0000-7000-8000-00000000ac3e/0199c9a0-0000-7000-8000-00000000dead/head
Authorization: Bearer {{access}}
HTTP 404

# 11.6 Revoking a token that jjforge doesn't know answers 200 too, so the
# answer tells a caller nothing.
POST {{server}}/oauth/revoke
[Form]
token: jfr_0199c9a1-7c00-7000-8000-000000000000_nothing
HTTP 200

# 11.7 A refresh token used twice ends its session, because only a copy can
# present it again.
GET {{server}}/oauth/authorize
[Query]
response_type: code
client_id: web
redirect_uri: {{server}}/callback
state: reuse-{{run}}
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
stolen: jsonpath "$.refresh_token"

POST {{server}}/oauth/token
[Form]
grant_type: refresh_token
client_id: web
refresh_token: {{stolen}}
HTTP 200
[Captures]
newest: jsonpath "$.refresh_token"
newest_access: jsonpath "$.access_token"

POST {{server}}/oauth/token
[Form]
grant_type: refresh_token
client_id: web
refresh_token: {{stolen}}
HTTP 400
[Asserts]
jsonpath "$.error" == "invalid_grant"

POST {{server}}/oauth/token
[Form]
grant_type: refresh_token
client_id: web
refresh_token: {{newest}}
HTTP 400
[Asserts]
jsonpath "$.error" == "invalid_grant"

GET {{server}}/api/v1/me
Authorization: Bearer {{newest_access}}
HTTP 401

# 11.8 Once its access token expires, vcs refuses it too. The token of 11.1
# is the oldest, so it expires first.
GET {{sync}}/sync/v1/0199c9a0-0000-7000-8000-00000000ac3e/0199c9a0-0000-7000-8000-00000000dead/head
Authorization: Bearer {{access}}
[Options]
delay: 66000
HTTP 401
[Asserts]
jsonpath "$.code" == "unauthenticated"

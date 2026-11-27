# pending
# A person signs in on a machine without a browser. `jf` asks for a device
# code, shows the person a short code, and asks again until the person has
# typed the code on another device and signed in there, as RFC 8628
# decides. Here the requests of `jf` and of the other device take turns.

# 9.1 jf starts a device sign-in for acme and gets the code to show.
POST {{server}}/oauth/device
[Form]
client_id: jf
org: acme
scope: read write
HTTP 200
[Captures]
device_code: jsonpath "$.device_code"
user_code: jsonpath "$.user_code"
[Asserts]
jsonpath "$.user_code" matches "^[BCDFGHJKLMNPQRSTVWXZ]{4}-[BCDFGHJKLMNPQRSTVWXZ]{4}$"
jsonpath "$.verification_uri" == "{{server}}/device"
jsonpath "$.verification_uri_complete" == "{{server}}/device?user_code={{user_code}}"
jsonpath "$.expires_in" == 900
jsonpath "$.interval" == 5

# 9.2 Until the person confirms, jf hears `authorization_pending`.
POST {{server}}/oauth/token
[Form]
grant_type: urn:ietf:params:oauth:grant-type:device_code
client_id: jf
device_code: {{device_code}}
HTTP 400
[Asserts]
jsonpath "$.error" == "authorization_pending"

# 9.3 A jf that asks again within the interval hears `slow_down`, and must
# wait 5 seconds longer from then on.
POST {{server}}/oauth/token
[Form]
grant_type: urn:ietf:params:oauth:grant-type:device_code
client_id: jf
device_code: {{device_code}}
HTTP 400
[Asserts]
jsonpath "$.error" == "slow_down"

# 9.4 On the other device, the person types the code. jjforge sends that
# browser to acme's provider.
POST {{server}}/oauth/device/confirm
[Form]
user_code: {{user_code}}
HTTP 302
[Captures]
provider: header "Location"
[Asserts]
header "Location" contains "/github/authorize?"
header "Location" contains "client_id=acme-jjforge"

# 9.5 The person signs in at the provider, which sends the browser back.
POST {{provider}}
[Form]
username: acme-member
HTTP 302
[Captures]
callback: header "Location"

# 9.6 The callback approves the device and tells the person they are done.
GET {{callback}}
HTTP 302
[Asserts]
header "Location" == "{{server}}/device?approved=true"

# 9.7 jf, waiting out its interval, gets its tokens, for the person who
# confirmed.
POST {{server}}/oauth/token
[Options]
delay: 11000
[Form]
grant_type: urn:ietf:params:oauth:grant-type:device_code
client_id: jf
device_code: {{device_code}}
HTTP 200
[Captures]
access: jsonpath "$.access_token"
[Asserts]
jsonpath "$.refresh_token" startsWith "jfr_"

GET {{server}}/api/v1/me
Authorization: Bearer {{access}}
HTTP 200
[Asserts]
jsonpath "$.name" == "acme-member"

# 9.8 A device code gives tokens once.
POST {{server}}/oauth/token
[Form]
grant_type: urn:ietf:params:oauth:grant-type:device_code
client_id: jf
device_code: {{device_code}}
HTTP 400
[Asserts]
jsonpath "$.error" == "invalid_grant"

# 9.9 A code that nobody issued sends the browser back to the page.
POST {{server}}/oauth/device/confirm
[Form]
user_code: QQQQ-QQQQ
HTTP 302
[Asserts]
header "Location" == "{{server}}/device?error=unknown_code"

# 9.10 A device sign-in needs an organization.
POST {{server}}/oauth/device
[Form]
client_id: jf
scope: read write
HTTP 400
[Asserts]
jsonpath "$.error" == "invalid_request"

# 9.11 Only jf signs in with a device code.
POST {{server}}/oauth/device
[Form]
client_id: web
org: acme
HTTP 400
[Asserts]
jsonpath "$.error" == "invalid_client"

# 9.12 When the person declines at the provider, jf hears `access_denied`.
POST {{server}}/oauth/device
[Form]
client_id: jf
org: acme
HTTP 200
[Captures]
device_code: jsonpath "$.device_code"
user_code: jsonpath "$.user_code"

POST {{server}}/oauth/device/confirm
[Form]
user_code: {{user_code}}
HTTP 302
[Captures]
upstream_state: header "Location" regex "[?&]state=([^&]+)"

GET {{server}}/oauth/callback
[Query]
state: {{upstream_state}}
error: access_denied
HTTP 302
[Asserts]
header "Location" == "{{server}}/device?error=access_denied"

POST {{server}}/oauth/token
[Form]
grant_type: urn:ietf:params:oauth:grant-type:device_code
client_id: jf
device_code: {{device_code}}
HTTP 400
[Asserts]
jsonpath "$.error" == "access_denied"

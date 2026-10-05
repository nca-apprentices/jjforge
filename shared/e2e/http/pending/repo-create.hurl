# An owner creates an empty repository, and members use it at once. The tokens
# come from the seed that `mise run up` loads once #6 and #13 are built: an
# owner and a member of the organization `acme`.

# An owner creates an empty repository and gets its clone address.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_owner}}
{
  "name": "demo-{{run}}"
}
HTTP 201
[Captures]
clone_url: jsonpath "$.cloneUrl"
[Asserts]
jsonpath "$.org" == "acme"
jsonpath "$.name" == "demo-{{run}}"
jsonpath "$.cloneUrl" matches "^https?://.+/sync/v1/[0-9a-f]+/[0-9a-f]+$"
jsonpath "$.createdAt" isIsoDate

# It can be cloned right away: the sync head answers.
GET {{clone_url}}/head
Authorization: Bearer {{acme_owner}}
HTTP 200
[Asserts]
header "ETag" exists

# A member reads it.
GET {{server}}/api/v1/orgs/acme/repos/demo-{{run}}
Authorization: Bearer {{acme_member}}
HTTP 200
[Asserts]
jsonpath "$.name" == "demo-{{run}}"

# A member can't create one: only owners do, as #15 decides.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_member}}
{
  "name": "denied-{{run}}"
}
HTTP 403
[Asserts]
jsonpath "$.code" == "forbidden"

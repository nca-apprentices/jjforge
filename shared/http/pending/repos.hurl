# satisfies #25 #26 #15
# An owner creates a repository, members see it, outsiders don't, and a name
# is unique within its organization. The tokens come from the seed that
# `mise run up` loads once E04 and E05 are built: an owner and a member of the
# organization `acme`, and an owner of `other`.

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

# The same name in the same organization is a conflict.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_owner}}
{
  "name": "demo-{{run}}"
}
HTTP 409
[Asserts]
header "Content-Type" == "application/problem+json"
jsonpath "$.code" == "name_taken"

# The same name in another organization succeeds.
POST {{server}}/api/v1/orgs/other/repos
Authorization: Bearer {{other_owner}}
{
  "name": "demo-{{run}}"
}
HTTP 201

# A member reads it.
GET {{server}}/api/v1/orgs/acme/repos/demo-{{run}}
Authorization: Bearer {{acme_member}}
HTTP 200
[Asserts]
jsonpath "$.name" == "demo-{{run}}"

# A member can't create one.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_member}}
{
  "name": "denied-{{run}}"
}
HTTP 403
[Asserts]
jsonpath "$.code" == "forbidden"

# Someone outside the organization doesn't see it.
GET {{server}}/api/v1/orgs/acme/repos/demo-{{run}}
Authorization: Bearer {{other_owner}}
HTTP 404
[Asserts]
jsonpath "$.code" == "not_found"

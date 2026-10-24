# pending
# An owner deletes a repository, and its name is free again.

# An owner creates a repository.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_owner}}
{
  "name": "gone-{{run}}"
}
HTTP 201

# 28.4 A member can't delete it: only owners do, as #15 decides.
DELETE {{server}}/api/v1/orgs/acme/repos/gone-{{run}}
Authorization: Bearer {{acme_member}}
HTTP 403
[Asserts]
jsonpath "$.code" == "forbidden"

# 28.1 The owner deletes it.
DELETE {{server}}/api/v1/orgs/acme/repos/gone-{{run}}
Authorization: Bearer {{acme_owner}}
HTTP 204

# 28.2 It can no longer be read.
GET {{server}}/api/v1/orgs/acme/repos/gone-{{run}}
Authorization: Bearer {{acme_owner}}
HTTP 404
[Asserts]
jsonpath "$.code" == "not_found"

# 28.3 Deleting it again is not found too.
DELETE {{server}}/api/v1/orgs/acme/repos/gone-{{run}}
Authorization: Bearer {{acme_owner}}
HTTP 404

# 28.2 Its name can be used again.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_owner}}
{
  "name": "gone-{{run}}"
}
HTTP 201

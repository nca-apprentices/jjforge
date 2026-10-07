# pending
# A repository name is unique within its organization, and free in another.

# An owner creates a repository.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_owner}}
{
  "name": "twice-{{run}}"
}
HTTP 201

# 26.1 The same name in the same organization is a conflict.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_owner}}
{
  "name": "twice-{{run}}"
}
HTTP 409
[Asserts]
header "Content-Type" == "application/problem+json"
jsonpath "$.code" == "name_taken"

# 26.2 The same name in another organization succeeds.
POST {{server}}/api/v1/orgs/other/repos
Authorization: Bearer {{other_owner}}
{
  "name": "twice-{{run}}"
}
HTTP 201

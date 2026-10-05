# A person sees only the organizations and repositories they belong to. An
# outsider learns nothing from the answer: not found, never forbidden.

# An owner of acme creates a repository.
POST {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_owner}}
{
  "name": "secret-{{run}}"
}
HTTP 201

# The owner of another organization doesn't see it.
GET {{server}}/api/v1/orgs/acme/repos/secret-{{run}}
Authorization: Bearer {{other_owner}}
HTTP 404
[Asserts]
jsonpath "$.code" == "not_found"

# Nor the organization itself, nor its repositories.
GET {{server}}/api/v1/orgs/acme
Authorization: Bearer {{other_owner}}
HTTP 404
[Asserts]
jsonpath "$.code" == "not_found"

GET {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{other_owner}}
HTTP 404

# Their list of organizations holds only their own.
GET {{server}}/api/v1/orgs
Authorization: Bearer {{other_owner}}
HTTP 200
[Asserts]
jsonpath "$.items[*].name" includes "other"
jsonpath "$.items[*].name" not includes "acme"

# A member of acme sees the repository in the list.
GET {{server}}/api/v1/orgs/acme/repos
Authorization: Bearer {{acme_member}}
HTTP 200
[Asserts]
jsonpath "$.items[*].name" includes "secret-{{run}}"

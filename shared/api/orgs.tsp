import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

namespace jjforge;

@tag("orgs")
@summary("List the organizations the principal belongs to")
@returnsDoc("One page of organizations.")
@extension("x-requirements", #["#27"])
@get
@route("/api/orgs")
op listOrgs(...Parameters.Paging): OrgPage | Unauthenticated;

@tag("orgs")
@summary("Create an organization, owned by the principal")
@extension("x-requirements", #["#24"])
@post
@route("/api/orgs")
op createOrg(@body body: OrgCreate):
  | Created<Org>
  | Invalid
  | Unauthenticated
  | Conflict;

@tag("orgs")
@summary("Show one organization")
@returnsDoc("The organization.")
@extension("x-requirements", #["#27"])
@get
@route("/api/orgs/{org}")
op getOrg(...Parameters.Org): Org | Unauthenticated | NotFound;

@tag("orgs")
@summary("Connect the organization's identity provider")
@returnsDoc("Verified and saved.")
@extension("x-requirements", #["#7"])
@put
@route("/api/orgs/{org}/identity-provider")
op setIdentityProvider(...Parameters.Org, @body body: IdentityProvider):
  | IdentityProvider
  | Invalid
  | Unauthenticated
  | Forbidden
  | NotFound;

@tag("orgs")
@summary("List the organization's members and their roles")
@returnsDoc("One page of members.")
@extension("x-requirements", #["#14", "#15"])
@get
@route("/api/orgs/{org}/members")
op listMembers(...Parameters.Org, ...Parameters.Paging):
  | MemberPage
  | Unauthenticated
  | NotFound;

@tag("orgs")
@summary("Add a member or change their role")
@returnsDoc("The member with their role.")
@extension("x-requirements", #["#14", "#15"])
@put
@route("/api/orgs/{org}/members/{principal}")
op putMember(
  ...Parameters.Org,
  ...Parameters.Principal,
  @body body: MemberUpdate,
): Member | Unauthenticated | Forbidden | NotFound;

/** Removing the last owner fails with `precondition_failed`. */
@tag("orgs")
@summary("Remove a member")
@returnsDoc("Removed.")
@extension("x-requirements", #["#14"])
@delete
@route("/api/orgs/{org}/members/{principal}")
op removeMember(...Parameters.Org, ...Parameters.Principal):
  | NoContentResponse
  | Unauthenticated
  | Forbidden
  | NotFound
  | Conflict;

@tag("orgs")
@summary("List the organization's access decisions, newest first")
@returnsDoc("One page of access records.")
@extension("x-requirements", #["#17"])
@get
@route("/api/orgs/{org}/access-records")
op listAccessRecords(...Parameters.Org, ...Parameters.Paging):
  | AccessRecordPage
  | Unauthenticated
  | Forbidden
  | NotFound;

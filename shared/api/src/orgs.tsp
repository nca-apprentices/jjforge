import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

namespace jjforge;

@tag("orgs")
@summary("List the organizations the principal belongs to")
@returnsDoc("One page of organizations.")
@get
@route("/api/v1/orgs")
op listOrgs(...Parameters.Paging): OrgPage | Unauthenticated;

@tag("orgs")
@summary("Create an organization, owned by the principal")
@post
@route("/api/v1/orgs")
op createOrg(@body body: OrgCreate):
  | Created<Org>
  | Invalid
  | Unauthenticated
  | Conflict;

@tag("orgs")
@summary("Show one organization")
@returnsDoc("The organization.")
@get
@route("/api/v1/orgs/{org}")
op getOrg(...Parameters.Org): Org | Unauthenticated | NotFound;

@tag("orgs")
@summary("Connect the organization's identity provider")
@returnsDoc("Verified and saved.")
@put
@route("/api/v1/orgs/{org}/identity-provider")
op setIdentityProvider(...Parameters.Org, @body body: IdentityProvider):
  | IdentityProvider
  | Invalid
  | Unauthenticated
  | Forbidden
  | NotFound;

@tag("orgs")
@summary("List the organization's members and their roles")
@returnsDoc("One page of members.")
@get
@route("/api/v1/orgs/{org}/members")
op listMembers(...Parameters.Org, ...Parameters.Paging):
  | MemberPage
  | Unauthenticated
  | NotFound;

@tag("orgs")
@summary("Add a member or change their role")
@returnsDoc("The member with their role.")
@put
@route("/api/v1/orgs/{org}/members/{principal}")
op putMember(
  ...Parameters.Org,
  ...Parameters.Principal,
  @body body: MemberUpdate,
): Member | Unauthenticated | Forbidden | NotFound;

/** Removing the last owner fails with `precondition_failed`. */
@tag("orgs")
@summary("Remove a member")
@returnsDoc("Removed.")
@delete
@route("/api/v1/orgs/{org}/members/{principal}")
op removeMember(...Parameters.Org, ...Parameters.Principal):
  | NoContentResponse
  | Unauthenticated
  | Forbidden
  | NotFound
  | Conflict;

@tag("orgs")
@summary("List the organization's access decisions, newest first")
@returnsDoc("One page of access records.")
@get
@route("/api/v1/orgs/{org}/access-records")
op listAccessRecords(...Parameters.Org, ...Parameters.Paging):
  | AccessRecordPage
  | Unauthenticated
  | Forbidden
  | NotFound;

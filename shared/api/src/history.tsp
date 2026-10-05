import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

namespace jjforge;

@tag("history")
@summary("List the changes reachable from a revision, newest first")
@returnsDoc("One page of changes.")
@get
@route("/api/v1/orgs/{org}/repos/{repo}/changes")
op listChanges(...Parameters.InRepo, ...Parameters.Rev, ...Parameters.Paging):
  | ChangePage
  | Unauthenticated
  | NotFound
  | Ambiguous;

/** `rev` is a change ID, a commit ID, a unique prefix of either, or a bookmark. */
@tag("history")
@summary("Show one change")
@returnsDoc("The change.")
@get
@route("/api/v1/orgs/{org}/repos/{repo}/changes/{rev}")
op getChange(...Parameters.InRepo, ...Parameters.RevPath):
  | Change
  | Unauthenticated
  | NotFound
  | Ambiguous;

@tag("history")
@summary("List the earlier versions of a change, newest first")
@returnsDoc("One page of versions.")
@get
@route("/api/v1/orgs/{org}/repos/{repo}/changes/{rev}/evolution")
op getEvolution(
  ...Parameters.InRepo,
  ...Parameters.RevPath,
  ...Parameters.Paging,
): VersionPage | Unauthenticated | NotFound | Ambiguous;

@tag("history")
@summary("List the operations that changed the repository, newest first")
@returnsDoc("One page of operations.")
@get
@route("/api/v1/orgs/{org}/repos/{repo}/operations")
op listOperations(...Parameters.InRepo, ...Parameters.Paging):
  | OperationPage
  | Unauthenticated
  | NotFound;

@tag("history")
@summary("List the bookmarks and their targets")
@returnsDoc("One page of bookmarks.")
@get
@route("/api/v1/orgs/{org}/repos/{repo}/bookmarks")
op listBookmarks(...Parameters.InRepo, ...Parameters.Paging):
  | BookmarkPage
  | Unauthenticated
  | NotFound;

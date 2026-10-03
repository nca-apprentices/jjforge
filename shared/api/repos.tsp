import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

namespace jjforge;

/** An event stream of `Activity`. */
model ActivityStream {
  @header contentType: "text/event-stream";
  @body body: string;
}

@tag("repos")
@summary("List the organization's repositories")
@returnsDoc("One page of repositories.")
@extension("x-requirements", #["#27"])
@get
@route("/api/v1/orgs/{org}/repos")
op listRepos(...Parameters.Org, ...Parameters.Paging):
  | RepoPage
  | Unauthenticated
  | NotFound;

@tag("repos")
@summary("Create an empty repository")
@extension("x-requirements", #["#25", "#26", "#15"])
@post
@route("/api/v1/orgs/{org}/repos")
op createRepo(...Parameters.Org, @body body: RepoCreate):
  | Created<Repo>
  | Invalid
  | Unauthenticated
  | Forbidden
  | NotFound
  | Conflict;

@tag("repos")
@summary("Show one repository and its clone address")
@returnsDoc("The repository.")
@extension("x-requirements", #["#27", "#52"])
@get
@route("/api/v1/orgs/{org}/repos/{repo}")
op getRepo(...Parameters.InRepo): Repo | Unauthenticated | NotFound;

@tag("repos")
@summary("Delete a repository and everything in it")
@returnsDoc("Deleted.")
@extension("x-requirements", #["#28", "#15"])
@delete
@route("/api/v1/orgs/{org}/repos/{repo}")
op deleteRepo(...Parameters.InRepo):
  | NoContentResponse
  | Unauthenticated
  | Forbidden
  | NotFound;

/**
 * Replays recent activity, newest last, then sends each new event as it
 * happens. Each event's `data` is one `Activity` as JSON. Reconnecting
 * with `Last-Event-ID` resumes after that event.
 */
@tag("repos")
@summary("Stream the repository's activity as server-sent events")
@extension("x-requirements", #["#49"])
@get
@route("/api/v1/orgs/{org}/repos/{repo}/activity")
op streamActivity(
  ...Parameters.InRepo,
  @header("Last-Event-ID") lastEventId?: string,
): ActivityStream | Unauthenticated | NotFound;

import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

namespace jjforge;

@tag("echo")
@summary("Echo a message through vcs")
@returnsDoc("The message vcs answered.")
@useAuth(NoAuth)
@post
@route("/api/v1/echo")
op echo(@body body: Echo): Echo | Invalid;

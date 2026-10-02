import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

namespace jjforge;

@tag("echo")
@summary("Echo a message through vcsd")
@returnsDoc("The message vcsd answered.")
@useAuth(NoAuth)
@post
@route("/api/echo")
op echo(@body body: Echo): Echo | Invalid;

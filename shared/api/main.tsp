// The REST contract. `mise run spec:build` compiles it to shared/openapi.yaml,
// which the server and the web app generate their code from.
//
// Every operation names the requirement issues it serves in x-requirements,
// and `mise run spec:trace` checks that each one is a Requirement. An
// operation that isn't built yet answers 501, see
// shared/docs/adr/0011-controllers-implement-contracts.md.
import "@typespec/http";
import "@typespec/openapi";
import "@typespec/openapi3";
import "./models.tsp";
import "./errors.tsp";
import "./parameters.tsp";
import "./echo.tsp";
import "./auth.tsp";
import "./orgs.tsp";
import "./repos.tsp";
import "./history.tsp";
import "./files.tsp";

using Http;
using OpenAPI;

/**
 * The public API of jjforge. The web app, the `jf` CLI, and agents all use it.
 * Only the sync protocol is exposed besides it: see
 * shared/docs/adr/0002-public-protocols.md.
 *
 * A list answers one page and a `next` cursor while more pages exist. Every
 * failure answers `application/problem+json` with a stable `code`.
 */
@service(#{ title: "jjforge" })
@info(#{
  version: "1",
  license: #{ name: "Apache-2.0", identifier: "Apache-2.0" },
})
@server("/", "The host that serves the web app")
@useAuth(bearer)
@tagMetadata(#[
  #{
    name: "echo",
    description: "The walking skeleton. Removed once a real operation runs end to end.",
  },
  #{
    name: "auth",
    description: "Sign-in, tokens, and the signed-in principal.",
  },
  #{
    name: "orgs",
    description: "Organizations, their members, and their access records.",
  },
  #{ name: "repos", description: "Repositories and their activity." },
  #{
    name: "history",
    description: "Changes, their evolution, operations, and bookmarks.",
  },
  #{ name: "files", description: "Trees, file contents, and diffs." }
])
namespace jjforge;

/** A token from `/oauth/token`. See shared/docs/adr/0004-identity-and-tokens.md. */
model bearer {
  type: AuthType.http;
  scheme: "bearer";
  bearerFormat: "Biscuit";
}

/** Created. */
model Created<T> {
  @statusCode status: 201;
  @body body: T;
}

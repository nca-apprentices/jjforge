# 0006. The REST contract is TypeSpec, served by generated controllers

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

The contract is written before the code that serves it. `shared/openapi.yaml`
was written by hand: every list repeated its page schema, every operation
repeated its error responses, and a wrong `$ref` surfaced only in Redocly or
in the generated code. On the server, a hand-written request mapping ties
nothing to the contract before a request arrives, and an operation that no
controller serves answers 404, which looks like a typo, not like work to do.

The options for the contract were one YAML file, YAML split into files and
bundled with Redocly, or TypeSpec compiled to OpenAPI. Templates such as
`Page<T>` remove the repetition and the compiler checks every reference, at
the cost of one more language and a compile step before every build. A trial
compiled to an OpenAPI file with no change that oasdiff reports, in 932 lines
against 1,298.

The options for the server were hand-written mappings checked at runtime,
generated interfaces with default methods where an unbuilt operation answers
501, or generated abstract interfaces where the compiler lists every
operation the server doesn't serve.

## Decision

- `shared/api/` holds the contract in TypeSpec. `mise run api:build` compiles
  it to `shared/openapi.yaml`, which git ignores and nobody edits. Every task
  that reads it depends on `api:build`, the server image compiles it in its
  web stage, and the release attaches it as an asset.
- `mise run api:lint` fails when a `.tsp` file isn't formatted, then lints
  the compiled file with Redocly.
- Operations stay in the `jjforge` namespace, each with its own `@tag` and
  full `@route`, because a nested namespace prefixes the operation ID and
  renames the generated methods. A spread of parameters names its namespace,
  as in `...Parameters.Org`.
- The server build generates kotlin-spring interfaces from the contract, with
  default methods while a tag has pending requirements, and abstract
  interfaces once the tag is served in full. A default method answers 501.
- Every `@RestController` implements a generated interface and maps no route
  of its own. Each tag has one controller, so every operation in the
  contract is routed. `ControllerContractTest` checks both directions: a
  controller maps only contract routes, and every generated interface has a
  controller.
- Kotlin compiles with the `no-compatibility` JVM default mode. The other
  modes copy each default method, with its route, into the controller.

## Consequences

- OpenAPI 3.1 has no way to name the event schema of a `text/event-stream`
  response, so `Activity` is reachable only through the description of
  `streamActivity`, until the generators support OpenAPI 3.2.
- Some output differs from the hand-written file without changing the
  generated code: error responses are repeated in each operation, operations
  without authentication carry `security: [{}]`, and a 204 response carries
  TypeSpec's default description.

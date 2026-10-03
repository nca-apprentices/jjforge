# 0016. The REST contract is written in TypeSpec

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

`shared/openapi.yaml` was written by hand. Every list repeated its page schema,
and every operation repeated its error responses. A wrong `$ref` surfaced only
in Redocly or in the generated code.

The options were:

- Keep one YAML file. Nothing new to learn, but the repetition grows with every
  operation.
- Split the YAML into files and bundle them with Redocly. Shorter diffs, but
  the same repetition.
- Write TypeSpec and compile it to OpenAPI. Templates such as `Page<T>` remove
  the repetition, and the compiler checks every reference. It is one more
  language, and the generators need the compiled YAML, so every build
  compiles it first.

A trial compiled the TypeSpec to an OpenAPI file with no change that oasdiff
reports and with the same generated Kotlin and TypeScript types. It took 932
lines against 1,298.

## Decision

- `shared/api/` holds the contract in TypeSpec. `mise run spec:build` compiles
  it to `shared/openapi.yaml`, which git ignores and nobody edits. Every task
  that reads it depends on `spec:build`, the server image compiles it in its
  web stage, and the release attaches it as an asset. A server build that
  starts without it, such as CodeQL's, compiles it itself.
- `mise run spec:lint` fails when a `.tsp` file isn't formatted, then lints
  the compiled file with Redocly.
- Operations stay in the `jjforge` namespace, each with its own `@tag` and full
  `@route`. TypeSpec prefixes the operation ID of an operation in a nested
  namespace or interface, as in `Orgs_listOrgs`, which renames the generated
  methods.
- A spread of path or query parameters names its namespace, as in
  `...Parameters.Org`. A bare `Org` resolves to the model of that name, and the
  compiler accepts the spread.

## Consequences

- OpenAPI 3.1 has no way to name the event schema of a `text/event-stream`
  response, so `Activity` is reachable only through the description of
  `streamActivity`. TypeSpec emits it for OpenAPI 3.2, once the generators
  support that version.
- Some output differs from the hand-written file without changing the
  generated code: error responses are repeated in each operation instead of shared, operations
  without authentication carry `security: [{}]`, and a 204 response carries
  TypeSpec's default description.

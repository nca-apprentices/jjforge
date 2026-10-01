# 0011. Controllers implement contracts

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

[openapi.yaml](../../openapi.yaml) is written before the code that serves it.
A hand-written request mapping ties nothing to the contract before a request
arrives, and an operation in the contract that no controller serves answers
404, which looks like a typo, not like work to do.

Options: hand-written mappings checked at runtime, which say nothing about an
operation nobody calls. Generated interfaces with default methods, where an
unbuilt operation answers 501. Generated abstract interfaces, where the
compiler lists every operation the server doesn't serve.

## Decision

Generated interfaces with default methods while a tag has pending
requirements, and abstract interfaces once the tag is served in full.

- The server build generates kotlin-spring interfaces from the contract. A
  default method answers 501 for an operation whose requirement is pending.
- Every `@RestController` implements a generated interface and maps no route
  of its own. A path the contract doesn't declare isn't served.
- Each tag has one controller, so every operation in the contract is routed.
  The first controllers are stubs that implement nothing.
- `ControllerContractTest` checks both directions: a controller maps only
  contract routes, and every generated interface has a controller.
- Kotlin compiles with the `no-compatibility` JVM default mode. The other modes
  copy each default method, with its route, into the controller.

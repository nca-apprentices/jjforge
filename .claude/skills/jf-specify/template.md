# Spec page template

Copy the block into `shared/docs/specs/<epic>-<slug>.md` and replace each
placeholder. Drop an optional section the epic doesn't need.

````markdown
# <Epic name>

<What the epic delivers, with a link to the epic.> <The module that owns it
and the ADRs its paths follow.>

## <What a person can do> (#<n>)

[#<n>](https://github.com/nca-apprentices/jjforge/issues/<n>) states the
requirement.

| ID      | Functional requirement                                              |
| ------- | ------------------------------------------------------------------- |
| <n>.1   | <Command or route> <does what>. <Refusal: status `problem code`.>    |

- Operations: <routes and `jf` commands>.
- Types: <proto messages, API schemas, and items of the Architecture and
  Libraries sections>.
- Flow: [<path>](#<path>).

Scenario: [<n>-<slug>.<hurl|bats>](../../e2e/<http|cli>/<n>-<slug>.<hurl|bats>).

## Out of scope

- <What a reader could expect, and the epic or decision that covers it.>

## Flow

### <Write path>

Operation `<operationId>` by principal P.

1. <Step>: <what happens>. Refused: <status> `<problem code>`.

## Persistence

<Object paths, tables with constraints and queries, and events with their
listeners.>

## Architecture

| Crate or module | Item   | Kind                       | Role               |
| --------------- | ------ | -------------------------- | ------------------ |
| <crate>         | <item> | <struct, trait, or method> | <what it does>     |

External systems and their twins in `shared/deploy/compose.yaml`: <system>
  as `<twin>`.

## Libraries

### <Library> <pinned version>

<Traits jjforge implements, types mapped field by field, calls made, and
parts left unused.>

## Security

- <What a path trusts, what it verifies, and the limit that bounds a hostile
  client.>

## Failures

<What each partial failure leaves behind, and who repairs it.>

## Tests

- <A test besides the scenarios, and what it proves.>

## Open questions

- <Question.> <The issue that answers it.>
````

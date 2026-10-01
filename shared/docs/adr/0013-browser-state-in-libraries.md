# 0013. Browser state belongs to a library

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

Three libraries already own state. TanStack Query owns what the API answered,
TanStack Router owns what the address selects, and TanStack Form owns a form.
State kept by hand with `useState` and `useEffect` contradicts them. A request
written as an effect, with a cancellation flag and a failure state, is a query
written by hand, and state kept in a component is lost when a link is shared
([#48](https://github.com/nca-apprentices/jjforge/issues/48)).

Options: keep hand-kept state and rely on review, forbid only `useEffect`, or
let a library own every kind of state.

## Decision

Every kind of browser state belongs to the library that owns it, and app and
feature code imports neither `useState` nor `useEffect` from React:

- Form state, field values, validation, and submission belong to TanStack
  Form.
- Source state, what a request answered, belongs to TanStack Query. A read
  that has to happen again names what changed in its query key.
- Address state, what the page selects, belongs to TanStack Router, so a
  shared link reproduces the page.

A lazy singleton, such as the router, is constructed outside any component.
The Biome rule `noRestrictedImports` names the two hooks, so a new occurrence fails
the build. Components in `web/shared/ui/` may hold presentation state that no
source or address describes, and each such exception is named in the rule,
not argued in review.

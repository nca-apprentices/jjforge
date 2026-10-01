# 0005. The server is a modular monolith

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

The building blocks grow at different speeds, and apprentices work on them
in parallel. Microservices from the start would add deployment and network
failure modes before any module needs them. A monolith without boundaries
grows into one tangle.

Options: microservices, a plain monolith, or a modular monolith with enforced
boundaries.

## Decision

- The server is one Spring Boot app, structured with Spring Modulith.
  Each module is a top-level package.
- Each module owns one Postgres schema. A module reads only its own tables.
- Modules talk through events. Events also go to Redpanda, so other services
  and a later split can consume them.
- The Spring Modulith verification runs as a test and fails the build on a
  dependency that crosses a module boundary.

## Consequences

A module can move into its own service later by consuming the same events and
taking its schema with it.

# 0012. The web app is split into apps, features, and shared packages

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

The web app grows a screen per building block, built by apprentices in
parallel. In one package, any file imports any other, and a boundary held only
by review erodes.

Options: one package with folder conventions, an Nx workspace with tagged
module boundaries, or a pnpm workspace whose boundaries Biome enforces. Nx adds
a second build tool to learn.

## Decision

- `web/` is a pnpm workspace with three kinds of package, each scoped
  `@jjforge/*`:
  - `apps/`: an app. It owns routing and composes features.
  - `features/`: one capability, such as a repository browser. It exposes a
    page and never routes.
  - `shared/`: code that every package may use, for example the API client.
- The Biome rule `noRestrictedImports` enforces the boundaries and fails the build:
  - No import climbs out of a package with a parent-relative path.
  - A shared package imports neither an app nor a feature.
  - A feature imports neither an app, another feature, nor
    `@tanstack/react-router`.
- Only `@jjforge/api` calls `fetch`. Apps and features use its generated
  client.
- The workspace uses pnpm and Biome only, no Nx.
- Every dependency version lives in the catalog in `web/pnpm-workspace.yaml`,
  and a `package.json` names `catalog:`. Strict catalog mode makes `pnpm add`
  write there too.

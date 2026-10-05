# 0008. The web app: packages, state, and markup

Status: accepted, 2026-10-05. Deciders: jjforge maintainers.

## Context

The web app grows a screen per building block, built by apprentices in
parallel. In one package, any file imports any other, and a boundary held only
by review erodes. Three libraries already own state. TanStack Query owns what
the API answered, TanStack Router owns what the address selects, and TanStack
Form owns a form. State kept by hand with `useState` and `useEffect`
contradicts them, and state kept in a component is lost when a link is shared
([#48](https://github.com/nca-apprentices/jjforge/issues/48)). The app needs
one look, light and dark, that meets WCAG 2.2 AA
([#50](https://github.com/nca-apprentices/jjforge/issues/50)), and when every
feature writes its own elements and classes, an accessibility fix has to be
made in every copy.

The options were folder conventions, an Nx workspace with tagged boundaries,
or a pnpm workspace whose boundaries Biome enforces. Nx adds a second build
tool to learn. For styling, a complete component library such as Mantine or
Primer React, whose internals belong to the library, or Tailwind CSS with
components in the shadcn/ui style, which the project copies in and owns.

## Decision

- `web/` is a pnpm workspace with three kinds of package, each scoped
  `@jjforge/*`. `apps/` owns routing and composes features. `features/` holds
  one capability each, exposes a page, and never routes. `shared/` holds code
  that every package may use.
- The Biome rule `noRestrictedImports` enforces the boundaries and fails the
  build. No import climbs out of a package with a parent-relative path. A
  shared package imports neither an app nor a feature. A feature imports
  neither an app, another feature, nor `@tanstack/react-router`.
- Only `@jjforge/api` calls `fetch`. Apps and features use its generated
  client.
- Every dependency version lives in the catalog in `web/pnpm-workspace.yaml`,
  and strict catalog mode makes `pnpm add` write there too. The workspace
  uses pnpm and Biome only.
- Every kind of browser state belongs to the library that owns it. Form state
  belongs to TanStack Form. Source state belongs to TanStack Query, and a
  read that has to happen again names what changed in its query key. Address
  state belongs to TanStack Router, so a shared link reproduces the page.
  App and feature code imports neither `useState` nor `useEffect`, and the
  same Biome rule fails the build on either. A lazy singleton, such as the
  router, is constructed outside any component.
- Only `@jjforge/ui` in `web/shared/ui` renders HTML elements, writes
  classes or styles, imports CSS, or uses Tailwind CSS v4. Apps and features
  compose its components. Two Biome GritQL plugins in `web/biome-plugins/`
  fail the build on an element or a `className` or `style` prop outside it.
- The design tokens are CSS variables after shadcn/ui's neutral theme, with a
  dark set for `prefers-color-scheme: dark`. Tailwind scans only
  `web/shared/ui`, and `@jjforge/ui/vite` gives an app the Vite plugin.
- A new look starts as a component in `@jjforge/ui`. Components there may
  hold presentation state that no source or address describes, and each
  such exception is named in the Biome rule, not argued in review. Radix
  primitives and the shadcn/ui CLI come in when the first component needs
  them.

# 0014. Only shared/ui renders markup and styles

Status: accepted, 2026-10-02. Deciders: jjforge maintainers.

## Context

The web app needs one look, light and dark, that meets WCAG 2.2 AA
([#50](https://github.com/nca-apprentices/jjforge/issues/50)). When every
feature writes its own HTML elements and classes, the look drifts, and an
accessibility fix has to be made in every copy.

Options for the styling framework:

- **Mantine or Primer React:** complete component libraries. The components'
  internals belong to the library, so a change that the library doesn't
  foresee needs a workaround.
- **Tailwind CSS with shadcn/ui:** utility classes, and component source that
  the project copies in and owns. Radix primitives supply accessible behavior
  where a component needs it. All of it is MIT licensed, built at build time,
  and widely documented.

## Decision

Tailwind CSS v4, with components written in the shadcn/ui style in
`web/shared/ui` (`@jjforge/ui`):

- Only `@jjforge/ui` renders HTML elements, writes classes or styles, imports
  CSS, or uses Tailwind. Apps and features compose its components.
- The design tokens are CSS variables after shadcn/ui's neutral theme, with a
  dark set for `prefers-color-scheme: dark`.
- Tailwind scans only `web/shared/ui`, and `@jjforge/ui/vite` gives an app the
  Vite plugin, so no app names Tailwind.
- Two Biome GritQL plugins in `web/biome-plugins/` fail the build on an HTML
  element or a `className` or `style` prop in an app or a feature.
  `noRestrictedImports` fails it on a CSS or Tailwind import outside
  `@jjforge/ui`.
- A new look starts as a component in `@jjforge/ui`. Radix primitives and the
  shadcn/ui CLI come in when the first component needs them.

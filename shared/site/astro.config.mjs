// The docs site, published to GitHub Pages. `pnpm sync` copies shared/docs
// into src/content/docs first, and the links validator fails the build on a
// link to a page or heading that doesn't exist.
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import mermaid from "astro-mermaid";
import starlightLinksValidator from "starlight-links-validator";

export default defineConfig({
  site: "https://jjforge-docs.nca-apprentices.dev",
  integrations: [
    // Renders ```mermaid fences in the browser. It must come before starlight.
    mermaid(),
    starlight({
      title: "jjforge",
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/nca-apprentices/jjforge" }],
      // The reading order of shared/docs/README.md: the pages for people who
      // use jjforge, then the pages for people who build it, then what both
      // look up, each kind collapsed.
      sidebar: [
        { label: "Overview", link: "/" },
        { label: "Using jjforge", items: ["concepts", "cli"] },
        { label: "Building jjforge", items: ["architecture", "workflow", "development"] },
        {
          label: "Reference",
          items: [
            { label: "Specs", collapsed: true, items: [{ autogenerate: { directory: "specs" } }] },
            { label: "Decisions", collapsed: true, items: [{ autogenerate: { directory: "adr" } }] },
          ],
        },
      ],
      plugins: [starlightLinksValidator()],
    }),
  ],
});

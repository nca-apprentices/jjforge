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
      // The reading order of shared/docs/README.md.
      sidebar: [
        { label: "Overview", link: "/" },
        {
          label: "The system",
          items: [
            "concepts",
            "architecture",
            "cli",
            { label: "Specs", items: [{ autogenerate: { directory: "specs" } }] },
            { label: "Architecture decisions", items: [{ autogenerate: { directory: "adr" } }] },
          ],
        },
        "planning",
        "development",
      ],
      plugins: [starlightLinksValidator()],
    }),
  ],
});

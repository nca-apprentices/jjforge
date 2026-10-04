// The docs site, published to GitHub Pages. `pnpm sync` copies shared/docs
// into src/content/docs first, and the links validator fails the build on a
// link to a page or heading that doesn't exist.
import { readdirSync } from "node:fs";
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import mermaid from "astro-mermaid";
import starlightLinksValidator from "starlight-links-validator";

// The sidebar lists the top-level docs, then the ADRs as one group.
const pages = readdirSync(new URL("../docs", import.meta.url))
  .filter((file) => file.endsWith(".md") && file !== "README.md")
  .map((file) => file.replace(/\.md$/, ""));

export default defineConfig({
  site: "https://jjforge-docs.nca-apprentices.dev",
  integrations: [
    // Renders ```mermaid fences in the browser. It must come before starlight.
    mermaid(),
    starlight({
      title: "jjforge",
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/nca-apprentices/jjforge" }],
      sidebar: [
        { label: "Overview", link: "/" },
        ...pages,
        { label: "Designs", items: [{ autogenerate: { directory: "design" } }] },
        { label: "Architecture decisions", items: [{ autogenerate: { directory: "adr" } }] },
      ],
      plugins: [starlightLinksValidator()],
    }),
  ],
});

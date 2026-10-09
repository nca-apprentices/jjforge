// Copies the Markdown in shared/docs into docs/, which Zensical builds. The
// docs stay plain Markdown that reads well on GitHub, so this:
//
// - points a relative link that leaves shared/docs at the file on GitHub, such
//   as ../proto/ -> https://github.com/nca-apprentices/jjforge/tree/main/shared/proto/,
// - fails when a page is missing from the nav in zensical.toml, which lists
//   every page by hand. The ADR template is linked but stays out of the nav.
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const GITHUB = "https://github.com/nca-apprentices/jjforge";
const ADR_TEMPLATE = "adr/0000-template.md";
const site = path.resolve(import.meta.dirname, "..");
const docs = path.resolve(site, "../docs");
const out = path.join(site, "docs");
const config = await readFile(path.join(site, "zensical.toml"), "utf8");

await rm(out, { recursive: true, force: true });

for (const file of await readdir(docs, { recursive: true })) {
  if (!file.endsWith(".md")) {
    continue;
  }

  if (file !== ADR_TEMPLATE && !config.includes(`"${file}"`)) {
    throw new Error(`${file}: add it to the nav in shared/site/zensical.toml`);
  }

  const text = await readFile(path.join(docs, file), "utf8");
  const target = path.join(out, file);
  await mkdir(path.dirname(target), { recursive: true });
  await writeFile(target, rewriteLinks(text, file));
}

// Rewrites inline links and link reference definitions outside code fences.
function rewriteLinks(text, file) {
  return text
    .split(/(^```[\s\S]*?^```$)/m)
    .map((part) =>
      part.startsWith("```")
        ? part
        : part
            .replace(/\]\(([^)\s]+)/g, (_, link) => `](${resolve(link, file)}`)
            .replace(/^(\[[^\]]+\]:\s*)(\S+)/gm, (_, label, link) => label + resolve(link, file)),
    )
    .join("");
}

function resolve(link, file) {
  if (/^([a-z]+:|#|\/)/i.test(link)) {
    return link;
  }

  const inDocs = path.posix.normalize(path.posix.join(path.posix.dirname(file), link));
  if (!inDocs.startsWith("../")) {
    return link;
  }

  const inRepo = path.posix.join("shared/docs", inDocs);
  const kind = link.endsWith("/") ? "tree" : "blob";
  return `${GITHUB}/${kind}/main/${inRepo}`;
}

// Copies the Markdown in shared/docs into src/content/docs, which Starlight
// builds. The docs stay plain Markdown that reads well on GitHub, so this:
//
// - turns the H1 into the `title` front matter Starlight needs,
// - renames README.md to index.md, the page for its directory,
// - points a link to another doc at its site route, such as
//   adr/0001-x.md#context -> /adr/0001-x/#context,
// - points any other relative link at the file on GitHub, such as
//   ../proto/ -> https://github.com/nca-apprentices/jjforge/blob/main/shared/proto/.
import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";

const GITHUB = "https://github.com/nca-apprentices/jjforge";
const site = path.resolve(import.meta.dirname, "..");
const repo = path.resolve(site, "../..");
const docs = path.join(repo, "shared/docs");
const out = path.join(site, "src/content/docs");

await rm(out, { recursive: true, force: true });

for (const file of await readdir(docs, { recursive: true })) {
  if (!file.endsWith(".md")) {
    continue;
  }

  const text = await readFile(path.join(docs, file), "utf8");
  const target = path.join(out, file.replace(/(^|\/)README\.md$/, "$1index.md"));
  await mkdir(path.dirname(target), { recursive: true });
  await writeFile(target, convert(text, file));
}

function convert(text, file) {
  const h1 = text.match(/^# (.+)\n+/m);
  if (!h1) {
    throw new Error(`${file}: no H1 to use as the title`);
  }

  const body = text.replace(h1[0], "");
  return `---\ntitle: ${JSON.stringify(h1[1])}\n---\n\n${rewriteLinks(body, file)}`;
}

// Rewrites inline links and link reference definitions outside code fences.
function rewriteLinks(body, file) {
  return body
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

  const [target, hash = ""] = link.split(/(?=#)/);
  const inDocs = path.posix.normalize(path.posix.join(path.posix.dirname(file), target));
  if (!inDocs.startsWith("../") && inDocs.endsWith(".md")) {
    // Starlight lowercases a route, so design/E10-x.md is served at /design/e10-x/.
    const route = inDocs.replace(/(^|\/)README\.md$/, "$1").replace(/\.md$/, "/").toLowerCase();
    return `/${route}${hash}`;
  }

  const inRepo = path.posix.join("shared/docs", inDocs);
  const kind = target.endsWith("/") ? "tree" : "blob";
  return `${GITHUB}/${kind}/main/${inRepo}${hash}`;
}

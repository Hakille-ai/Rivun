import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";

const root = process.cwd();
const srcDir = path.join(root, "src");
const docsDir = path.join(srcDir, "app", "docs");
const ignoredPrefixes = ["http://", "https://", "mailto:", "#"];
const hrefPattern = /(?:href=|href:\s*)["'`]([^"'`]+)["'`]/g;

function walk(dir) {
  return readdirSync(dir).flatMap((entry) => {
    const absolute = path.join(dir, entry);
    if (statSync(absolute).isDirectory()) {
      return walk(absolute);
    }
    return /\.(tsx?|jsx?|mdx?)$/.test(entry) ? [absolute] : [];
  });
}

function pageForDocHref(href) {
  const cleanHref = href.split("#")[0].split("?")[0];
  if (cleanHref === "/docs") {
    return path.join(docsDir, "page.tsx");
  }
  const slug = cleanHref.replace(/^\/docs\/?/, "");
  return path.join(docsDir, slug, "page.tsx");
}

const failures = [];
const checked = new Set();

for (const file of walk(srcDir)) {
  const body = readFileSync(file, "utf8");
  for (const match of body.matchAll(hrefPattern)) {
    const href = match[1];
    if (!href.startsWith("/docs") || ignoredPrefixes.some((prefix) => href.startsWith(prefix))) {
      continue;
    }
    const page = pageForDocHref(href);
    checked.add(href.split("#")[0].split("?")[0]);
    if (!existsSync(page)) {
      failures.push(`${path.relative(root, file)} links to ${href}, but ${path.relative(root, page)} does not exist`);
    }
  }
}

if (failures.length > 0) {
  console.error("Broken internal docs links:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(`Checked ${checked.size} internal docs routes.`);

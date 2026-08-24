// Walk every internal link from the front page and check what comes back.
//
//   npm run crawl
//   npm run crawl -- --lang it     also crawl from /it/
//
// Fails on: any 404 or 5xx; any page that came back as a raw template-failure
// dump; any page missing the site's own stylesheet or its main navigation.
//
// The raw-dump check is the reason this exists rather than a link checker. When
// a Tera template fails to parse, the kernel does not serve an error — it falls
// back to writing the item's fields into a bare `<html><body>` with no head, no
// navigation and no styling, and returns it with a 200. A crawler that only
// looks at status codes reports a perfectly healthy site.
import { chromium } from "playwright";
import { BASE } from "./pages.mjs";

const startPaths = ["/"];
if (process.argv.includes("--lang")) {
  const lang = process.argv[process.argv.indexOf("--lang") + 1];
  if (lang) startPaths.push(`/${lang}/`);
}

// Paths that are not pages: they are documents, or they change state.
const SKIP = [/^\/user\/logout/, /^\/admin/, /^\/api\//, /^\/cron\//];
const DOCUMENT = [/\.xml$/, /\.txt$/, /^\/llms\.txt$/];

const origin = new URL(BASE).origin;
const seen = new Set();
const queue = [...startPaths];
const problems = [];
const pages = [];

const browser = await chromium.launch();
const context = await browser.newContext();
const page = await context.newPage();

while (queue.length) {
  const path = queue.shift();
  if (seen.has(path)) continue;
  seen.add(path);
  if (SKIP.some((re) => re.test(path))) continue;

  const response = await page.goto(origin + path, { waitUntil: "domcontentloaded" });
  const status = response?.status() ?? 0;

  if (status >= 400) {
    problems.push(`${status}  ${path}`);
    pages.push({ path, status, ok: false });
    continue;
  }

  const isDocument = DOCUMENT.some((re) => re.test(path));
  if (isDocument) {
    pages.push({ path, status, ok: true, kind: "document" });
    continue;
  }

  const shape = await page.evaluate(() => ({
    hasSiteCss: [...document.styleSheets].some((s) => (s.href ?? "").endsWith("/static/css/site.css")),
    hasNav: !!document.querySelector("nav[aria-label='Main'] a"),
    hasMain: !!document.querySelector("main#main-content"),
    hasSkipLink: !!document.querySelector("a.skip-link"),
    title: document.title,
    links: [...document.querySelectorAll("a[href]")].map((a) => a.getAttribute("href")),
  }));

  // The signature of a fallback dump: content, but none of the document the
  // site's own base.html builds around it.
  if (!shape.hasMain || !shape.hasSiteCss) {
    problems.push(`RAW DUMP or unthemed  ${path}  (main=${shape.hasMain} css=${shape.hasSiteCss})`);
  } else if (!shape.hasNav) {
    problems.push(`no main navigation  ${path}`);
  } else if (!shape.hasSkipLink) {
    problems.push(`no skip link  ${path}`);
  }

  pages.push({ path, status, ok: true, title: shape.title });

  for (const href of shape.links) {
    if (!href || href.startsWith("#") || href.startsWith("mailto:")) continue;
    let url;
    try {
      url = new URL(href, origin + path);
    } catch {
      problems.push(`unparseable href "${href}" on ${path}`);
      continue;
    }
    if (url.origin !== origin) continue;
    const next = url.pathname + url.search;
    if (!seen.has(next)) queue.push(next);
  }
}

await browser.close();

for (const p of pages.sort((a, b) => a.path.localeCompare(b.path))) {
  console.log(`${String(p.status).padEnd(4)} ${p.ok ? "ok " : "BAD"}  ${p.path}${p.title ? `  — ${p.title}` : ""}`);
}

console.log(`\n${pages.length} pages crawled`);
if (problems.length) {
  console.error(`\n${problems.length} problem(s):`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
console.log("no 404s, no raw dumps, every page themed and navigable");

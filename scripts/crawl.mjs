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

const startPaths = ["/", "/llms.txt"];
if (process.argv.includes("--lang")) {
  const lang = process.argv[process.argv.indexOf("--lang") + 1];
  if (lang) startPaths.push(`/${lang}/`);
}

// Paths that are not pages: they are documents, or they change state.
const SKIP = [/^\/user\/logout/, /^\/admin/, /^\/api\//, /^\/cron\//];
// Not pages: they are documents, and have no navigation to check for.
const DOCUMENT = [/\.xml$/, /\.txt$/, /^\/llms\.txt$/, /^\/learn\/raw\//];

const origin = new URL(BASE).origin;
const seen = new Set();
const queue = [...startPaths];
const problems = [];
const pages = [];

const browser = await chromium.launch();
const context = await browser.newContext();
const page = await context.newPage();

// Fetch documents and nothing else. Two reasons, and the second is the one that
// matters: the crawl checks a page's structure, which is in its HTML, so the
// stylesheets and fonts are not needed; and the kernel rate-limits every GET,
// static assets included, at 100 a minute per IP with no way to configure it, so
// a crawl that also pulled ten subresources per page would 429 itself before it
// had seen a quarter of the site. Measured, and recorded in docs/LEDGER.md.
await page.route("**/*", (route) => {
  const type = route.request().resourceType();
  if (type === "document") {
    route.continue();
  } else {
    route.abort();
  }
});

// Even one request per page can outrun the limiter on a large crawl. A 429 is
// answered the way the header asks rather than treated as a failure.
async function fetchPage(url) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const response = await page.goto(url, { waitUntil: "domcontentloaded" });
    if (response?.status() !== 429) return response;
    const wait = Number(response.headers()["retry-after"] ?? 60);
    console.log(`      429, waiting ${wait}s before retrying ${url}`);
    await new Promise((r) => setTimeout(r, (wait + 1) * 1000));
  }
  return page.goto(url, { waitUntil: "domcontentloaded" });
}

while (queue.length) {
  const path = queue.shift();
  if (seen.has(path)) continue;
  seen.add(path);
  if (SKIP.some((re) => re.test(path))) continue;

  const response = await fetchPage(origin + path);
  const status = response?.status() ?? 0;

  if (status >= 400) {
    problems.push(`${status}  ${path}`);
    pages.push({ path, status, ok: false });
    continue;
  }

  const isDocument = DOCUMENT.some((re) => re.test(path));
  if (isDocument) {
    pages.push({ path, status, ok: true, kind: "document" });

    // llms.txt is the one document whose links have to be followed. It is the
    // machine-readable index of the site, so a link in it that 404s is a page a
    // reader was told about and cannot reach — and nothing else on the site
    // links to those URLs, so no other pass would find it.
    if (path === "/llms.txt") {
      const text = await page.evaluate(() => document.body.innerText);
      for (const match of text.matchAll(/https?:\/\/[^\s)]+/g)) {
        let url;
        try {
          url = new URL(match[0]);
        } catch {
          continue;
        }
        // The file names the production host; the crawl runs against a local
        // one. Same paths, different origin, so compare on the path.
        if (url.hostname !== "trovato.rs" && url.origin !== origin) continue;
        const next = url.pathname;
        if (!seen.has(next)) queue.push(next);
      }
    }
    continue;
  }

  const shape = await page.evaluate(() => ({
    // The link, not the loaded sheet: subresources are not fetched here.
    hasSiteCss: !!document.querySelector('link[rel="stylesheet"][href="/static/css/site.css"]'),
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

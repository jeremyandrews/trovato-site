// Walk every internal link from the front page and check what comes back.
//
//   npm run crawl
//   npm run crawl -- --lang it     also crawl from /it/
//
// Fails on: any 404 or 5xx; any page that came back as a raw template-failure
// dump; any page missing the site's own stylesheet or its main navigation; and
// any footer language switcher that does not round trip (see the round trip
// below, which is where the language switcher's coverage lives now).
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
  // `/${lang}/` — the address a translated site is most often entered on, and
  // the one a reader who picks the switcher on the front page lands on. On
  // 0.101.0 it was a 404 (a language prefix reached content aliases and nothing
  // else) and the front page could not be translated either, so the Italian had
  // to start at `/${lang}/why` instead. Both are fixed on 0.102.0.
  if (lang) startPaths.push(`/${lang}/`);
}

// Paths that are not pages: they are documents, or they change state.
const SKIP = [/^\/user\/logout/, /^\/admin/, /^\/api\//, /^\/cron\//];
// Not pages: they are documents, and have no navigation to check for.
//
// Images are in the list for the front page's gallery, whose figures link each
// screenshot to the full-size file so a reader can open one and actually read
// it. That is a link to a document, and demanding a skip link and a main
// landmark of a PNG would be the check misreading its own subject. The status
// is still checked, so a gallery pointing at a file that is not there is still
// a failed crawl.
const DOCUMENT = [
  /\.xml$/,
  /\.txt$/,
  /^\/llms\.txt$/,
  /^\/learn\/raw\//,
  /\.(png|jpe?g|gif|svg|webp|avif|ico|pdf|woff2?)$/i,
];

// The pages that are translated, and must therefore offer a real switcher.
//
// Named one by one, and deliberately as a positive list. The set is not a shape
// a test can infer: a page carries a switcher when the kernel hands the theme
// more than one entry in `available_translations`, which happens when, and only
// when, the item is translated. Everything else on the site renders the
// entry-point link instead, and correctly so — an untranslated blog post has no
// Italian address to offer, and a switcher pointing at one would be a lie.
//
// So "rendered the fallback" is not by itself a fault, and a check that treated
// it as one would fail on 84 honest pages. What is a fault is one of THESE
// pages rendering the fallback, because that is what a broken translation looks
// like from the outside, and the whole reason to write the list down is that the
// loss would otherwise be indistinguishable from the ordinary case.
//
// The five items are the five in
// config/variable.plugin.trovato_site.site_translations.yml, at both of their
// reader-facing addresses. Translating a sixth item means adding two lines here,
// and the gate will say so if you forget.
//
// Two page kinds are out of scope on purpose and are not in this list even
// though the site serves them in Italian:
//
//   * gather listings   — /news, /blog, /blog/archive, /learn, /authors/*
//   * themed plugin pages — /search, /contact, /user/login, /user/register,
//                           /user/recover
//
// The kernel reaches the theme on both with `active_language` set and no
// `available_translations` at all, so page.html has nothing to build a per-page
// switcher out of. That is a kernel gap, recorded as a finding and repeated in
// the template's own comment; it is not something this site can fix from a
// template, and it is not something this gate can assert its way out of.
const MUST_OFFER_A_SWITCHER = [
  "/",
  "/why",
  "/get-started",
  "/community",
  "/accessibility",
  "/it/",
  "/it/why",
  "/it/get-started",
  "/it/community",
  "/it/accessibility",
];

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
    // The language the page says it is, and the switcher it offers. The two
    // shapes are different elements on purpose in page.html: a real per-page
    // switcher is a <nav>, and the entry-point fallback is a <p>. Reading the
    // distinction here rather than inferring it from the link count means a
    // page that silently loses its switcher is a page that renders the
    // fallback, which the round trip below treats as a failure unless the page
    // is named as one the kernel supplies no translations for.
    htmlLang: document.documentElement.getAttribute("lang"),
    switcher: [...document.querySelectorAll("nav.site-footer__language a")].map((a) => ({
      href: a.getAttribute("href"),
      hreflang: a.getAttribute("hreflang"),
    })),
    entryPointOnly: !!document.querySelector("p.site-footer__language a"),
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

  pages.push({
    path,
    status,
    ok: true,
    title: shape.title,
    htmlLang: shape.htmlLang,
    switcher: shape.switcher,
    entryPointOnly: shape.entryPointOnly,
    // Recorded rather than re-derived, so the round trip below can assert it
    // for itself instead of trusting that the check above already complained.
    themed: shape.hasMain && shape.hasSiteCss,
  });

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

// The language switcher, checked as a round trip.
//
// This replaces `the_language_switcher_links_are_real_addresses_in_both_directions`,
// which lived in checks/tests/content.rs until the 0.102.0 bump and could not be
// ported. That test read a `nav.english_path` out of the translations config and
// compared it to the item's alias. Both inputs are gone: the switcher's addresses
// come from the kernel's `available_translations` at render time, so nothing a
// test can read out of `config/` knows what a switcher will say. The only place
// the claim is still observable is a running site, which is here.
//
// Four assertions per link, which together are the thing the old test proved:
//
//   1. the address serves 200,
//   2. it is a real page and not a template-failure dump,
//   3. the page it lands on declares the language the link claimed, and
//   4. that page's own switcher points back where the crawl came from.
//
// (4) is what makes it a round trip rather than a link check. A switcher that
// sends an English reader to the Italian page but offers no way back is exactly
// the trap the original test existed to catch.
//
// The link is followed by reading the crawl's own visit to it. Every switcher
// href is an internal <a href> on a crawled page, so the crawl has already
// queued and fetched it; re-fetching here would double a 232-page crawl against
// a kernel that rate-limits at 100 GETs a minute and would tell us nothing new.
// The target being absent from the crawl is itself a failure and says so.
const byPath = new Map(pages.map((p) => [p.path, p]));
let switcherLinks = 0;

// A page that is required to offer a switcher but was never reached at all would
// otherwise pass by being absent: the loop below only sees pages the crawl
// visited. Unreachable is a worse failure than unswitched, so it is checked
// first and separately.
for (const path of MUST_OFFER_A_SWITCHER) {
  if (!byPath.has(path)) {
    problems.push(`translated page not reachable from the front page at all  ${path}`);
  }
}

for (const p of pages) {
  if (p.kind === "document" || !p.ok) continue;

  const required = MUST_OFFER_A_SWITCHER.includes(p.path);

  if (p.switcher.length === 0) {
    if (required) {
      problems.push(
        `translated page offers no language switcher  ${p.path}  ` +
          `(rendered ${p.entryPointOnly ? "the entry-point fallback" : "nothing at all"}; ` +
          `the kernel gave the theme no second entry in available_translations)`,
      );
    }
    // Everything else legitimately has no translation to point at.
    continue;
  }

  for (const link of p.switcher) {
    switcherLinks++;

    if (!link.href || !link.hreflang) {
      problems.push(`switcher link missing href or hreflang  ${p.path}`);
      continue;
    }

    let target;
    try {
      target = new URL(link.href, origin + p.path);
    } catch {
      problems.push(`switcher href unparseable "${link.href}"  ${p.path}`);
      continue;
    }
    if (target.origin !== origin) {
      problems.push(`switcher points off site "${link.href}"  ${p.path}`);
      continue;
    }

    const landing = byPath.get(target.pathname + target.search);
    if (!landing) {
      problems.push(`switcher target never crawled "${link.href}"  ${p.path}`);
      continue;
    }

    // 1 and 2: it serves, and it is a page rather than a failure dump. The crawl
    // has already recorded a raw dump as its own problem, so this only has to
    // refuse to call the round trip good.
    if (landing.status !== 200) {
      problems.push(`switcher target ${landing.status}  ${p.path} -> ${landing.path}`);
      continue;
    }
    if (landing.kind === "document") {
      problems.push(`switcher target is not a page  ${p.path} -> ${landing.path}`);
      continue;
    }
    if (!landing.themed) {
      problems.push(`switcher target is a raw dump  ${p.path} -> ${landing.path}`);
      continue;
    }

    // 3: it is in the language the link promised.
    if (landing.htmlLang !== link.hreflang) {
      problems.push(
        `switcher promised ${link.hreflang} and got <html lang="${landing.htmlLang}">  ` +
          `${p.path} -> ${landing.path}`,
      );
      continue;
    }

    // 4: and it offers the way back.
    const back = landing.switcher.some((l) => {
      try {
        return new URL(l.href, origin + landing.path).pathname === p.path;
      } catch {
        return false;
      }
    });
    if (!back) {
      problems.push(`switcher does not come back  ${p.path} -> ${landing.path} -> (nothing to ${p.path})`);
    }
  }
}

for (const p of pages.sort((a, b) => a.path.localeCompare(b.path))) {
  console.log(`${String(p.status).padEnd(4)} ${p.ok ? "ok " : "BAD"}  ${p.path}${p.title ? `  — ${p.title}` : ""}`);
}

console.log(`\n${pages.length} pages crawled`);
console.log(`${switcherLinks} language switcher link(s) round-tripped`);
if (problems.length) {
  console.error(`\n${problems.length} problem(s):`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
console.log("no 404s, no raw dumps, every page themed and navigable");
console.log("every language switcher link serves, lands in the language it claimed, and comes back");

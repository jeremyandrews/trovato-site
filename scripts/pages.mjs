// The page set every browser-driven check runs over.
//
// One list, so a screenshot pass, an accessibility pass and a crawl all cover
// the same pages, and adding a page to the site means adding it in one place.
// Each entry names the template family it exercises: the point of the set is
// coverage of templates, not of URLs, and two pages that render through the same
// template only pay for one of them.
export const PAGES = [
  { path: "/",         name: "front",    family: "page--front.html" },
  { path: "/why",      name: "why",      family: "elements/item--page" },
  { path: "/news",     name: "news",     family: "gather/query--*" },
  { path: "/blog",     name: "blog",     family: "gather/query--blog_listing" },
  { path: "/contact",  name: "contact",  family: "plugin page + form" },
  // With a query, so the in-browser results render and every check covers
  // them: an excerpt quoting a long URL once pushed this page to 614px at a
  // 390px viewport, and the page without a query showed no results to catch it.
  //
  // `ready` is what must be on the page before it is checked: the in-browser
  // result cards, and the server results hidden behind them. The server
  // results render for this URL whether or not the in-browser search works, so
  // without it a broken index would leave the gates checking the fallback and
  // passing.
  {
    path: "/search?q=Drupal", name: "search", family: "search.html",
    ready: [".scolta-result-card", "#search-fallback[hidden]"],
  },
  { path: "/user/login", name: "login",  family: "user/*" },
  { path: "/learn",    name: "learn",    family: "gather/query--trovato_site.docs_index" },
  { path: "/learn/tutorial-01-hello-trovato", name: "docs", family: "elements/item--docs" },
  // The Italian render of the front page. It is here despite the set being a
  // set of template families, because language is the one thing a second page
  // through the same template does change: `<html lang>`, the switcher's
  // direction, and every string the template picks on `active_language`. Until
  // this entry existed, axe had never run against an Italian page at all,
  // though the site has shipped five of them since Gate 9.
  //
  // The front page rather than one of the other four because it renders the
  // most: the hero, the proof sections, the gallery and the listing, so it is
  // the Italian page with the most that can be wrong.
  //
  // What this does not buy: axe has no language detection, so it cannot see an
  // English string sitting inside an Italian page. The two known cases — the
  // English heading over the front page's listing of recent posts, and the
  // English slogan under the wordmark — are findings 3 and 7 in docs/REPORT.md
  // and stay invisible to this gate. What it does buy is everything structural:
  // `<html lang>`, contrast in both schemes, names and roles, on a render that
  // no automated check had ever visited.
  { path: "/it/",     name: "front-it", family: "page--front.html (it)" },
];

export const BASE = process.env.SITE_BASE ?? "http://127.0.0.1:3080";

// Options for chromium.launch(). PLAYWRIGHT_CHANNEL=chrome runs the installed
// Google Chrome instead of Playwright's bundled Chromium. On macOS the bundled
// build is not granted Local Network access, so it cannot reach a LAN address
// such as the k3s review site (net::ERR_ADDRESS_UNREACHABLE) while Chrome can.
// Unset, the bundled Chromium is used, as before.
export const LAUNCH = process.env.PLAYWRIGHT_CHANNEL
  ? { channel: process.env.PLAYWRIGHT_CHANNEL }
  : {};

// How long a page has to show everything its `ready` list names.
export const READY_TIMEOUT_MS = 15_000;

// Wait until every selector in the entry's `ready` list matches. Returns null
// when the page is ready (or has no list), else the selectors still missing.
export async function waitReady(page, entry) {
  if (!entry.ready) return null;
  try {
    await page.waitForFunction(
      (selectors) => selectors.every((s) => document.querySelector(s)),
      entry.ready,
      { timeout: READY_TIMEOUT_MS },
    );
    return null;
  } catch {
    return await page.evaluate(
      (selectors) => selectors.filter((s) => !document.querySelector(s)),
      entry.ready,
    );
  }
}

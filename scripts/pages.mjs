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
  { path: "/search",   name: "search",   family: "search.html" },
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

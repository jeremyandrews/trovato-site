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
];

export const BASE = process.env.SITE_BASE ?? "http://127.0.0.1:3080";

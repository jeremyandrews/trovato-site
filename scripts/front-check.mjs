// What the front page must and must not contain, as a pure function over the
// served HTML, so the rules have tests of their own (scripts/test/front.test.mjs)
// and scripts/front.mjs only has to fetch.
//
// Why this exists. Kernel 0.105.0 serves `/` through the ordinary item route,
// which renders every plain string field into `children` as a labelled,
// escaped `<div class="field">`. The front page template printed `children`,
// so below the gallery the page printed its own body and proof as visible
// markup, under "Body:" and "proof:", and a breadcrumb sat over the hero. The crawl, the accessibility pass and the
// screenshots all passed, because each of them checks that things are there
// and none of them checks that a thing is there once. This does.

// Markup that must never reach the page as text, and the labels the item
// route puts in front of a plain field.
const FORBIDDEN = [
  { needle: "&lt;p", why: "escaped <p> markup" },
  { needle: "&lt;section", why: "escaped <section> markup" },
  { needle: "field__label", why: "a field label from the item route's field rendering" },
  { needle: "Body:", why: "the body field's label" },
  { needle: "proof:", why: "the proof field's label" },
  { needle: 'class="breadcrumb"', why: "a breadcrumb" },
];

// Each part of the page, and how many times it must appear.
const PARTS = [
  { re: /<header class="front-hero"/g, want: 1, what: "the hero" },
  { re: /<a class="feature-card"/g, want: 8, what: "claim cards" },
  { re: /<section class="front-gallery"/g, want: 1, what: "the gallery" },
  { re: /<section class="front-listing"/g, want: 1, what: "the recent posts listing" },
];

function count(haystack, needle) {
  if (typeof needle === "string") return haystack.split(needle).length - 1;
  return (haystack.match(needle) ?? []).length;
}

/** The hero's lede sentence, or null when the page has none. */
export function ledeOf(html) {
  const m = html.match(/<p class="front-hero__lede">([^<]+)<\/p>/);
  return m ? m[1].trim() : null;
}

/**
 * The problems with one served front page, as readable lines; empty is good.
 *
 * `lang` is the language the page was asked for. The document element has to
 * name it, so the English page served at `/it/` fails here rather than passing
 * as a second copy of a correct page.
 */
export function frontPageProblems(html, lang) {
  const problems = [];
  const htmlLang = html.match(/<html[^>]*\slang="([^"]*)"/)?.[1];
  if (htmlLang !== lang) problems.push(`<html lang> is "${htmlLang ?? ""}", expected "${lang}"`);
  const start = html.indexOf("<main");
  const end = html.lastIndexOf("</main>");
  if (start < 0 || end < start) return ["no <main> element"];
  const main = html.slice(start, end);

  for (const { needle, why } of FORBIDDEN) {
    const n = count(html, needle);
    if (n > 0) problems.push(`${why}: "${needle}" appears ${n} time(s)`);
  }

  for (const { re, want, what } of PARTS) {
    const n = count(main, re);
    if (n !== want) problems.push(`${what}: expected ${want}, found ${n}`);
  }

  // The lede, read from the page itself so the same rule holds in every
  // language. Counted inside <main> only: the site header prints the same
  // sentence as its tagline, which is correct and not a repeat.
  const lede = ledeOf(main);
  if (!lede) {
    problems.push("no lede paragraph in the hero");
  } else {
    const n = count(main, lede);
    if (n !== 1) problems.push(`the lede "${lede}" appears ${n} times in <main>`);
  }

  return problems;
}

import { test } from "node:test";
import assert from "node:assert/strict";
import { frontPageProblems } from "../front-check.mjs";

const LEDE = "A content management system written in Rust.";
const card = '<a class="feature-card" href="/x"><h2>t</h2></a>';

// A front page as the site means it to be: header tagline, then one of each part.
function page(extra = "", lang = "en") {
  return `<html lang="${lang}" dir="ltr"><body>
<header class="site-header"><span>${LEDE}</span></header>
<main id="main-content" class="main">
<header class="front-hero"><p class="front-hero__lede">${LEDE}</p></header>
<section class="front-features">${card.repeat(8)}</section>
<section class="front-gallery"><h2>What it looks like</h2></section>
${extra}
<section class="front-listing"><h2>Recent posts</h2></section>
</main></body></html>`;
}

test("a correct front page has no problems, the header tagline included", () => {
  assert.deepEqual(frontPageProblems(page(), "en"), []);
});

test("the fields printed again through children fail", () => {
  // What the item route on 0.105.0 put below the gallery.
  const children =
    '<div class="field field-body"><strong class="field__label">Body:</strong> ' +
    `&lt;p class=&quot;front-hero__lede&quot;&gt;${LEDE}&lt;/p&gt;</div>` +
    '<div class="field field-proof"><strong class="field__label">proof:</strong> ' +
    "&lt;section class=&quot;front-features&quot;&gt;</div>";
  const problems = frontPageProblems(page(children), "en").join("\n");
  for (const needle of ["&lt;p", "&lt;section", "field__label", "Body:", "proof:", "appears 2 times"]) {
    assert.match(problems, new RegExp(needle.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
});

test("a breadcrumb fails", () => {
  assert.notDeepEqual(frontPageProblems(page('<nav class="breadcrumb"></nav>'), "en"), []);
});

test("the English page served for Italian fails", () => {
  const problems = frontPageProblems(page(), "it");
  assert.equal(problems.length, 1);
  assert.match(problems[0], /<html lang> is "en", expected "it"/);
  assert.deepEqual(frontPageProblems(page("", "it"), "it"), []);
});

test("a missing or doubled part fails", () => {
  assert.notDeepEqual(frontPageProblems(page().replace('<section class="front-gallery">', "<section>"), "en"), []);
  assert.notDeepEqual(frontPageProblems(page(card), "en"), []);
});

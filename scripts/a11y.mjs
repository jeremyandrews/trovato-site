// Run axe-core against the site and fail on anything it finds.
//
//   npm run a11y
//
// The page set is scripts/pages.mjs, chosen to cover every template family
// rather than every URL: two pages that render through the same template pay for
// one of them. Each page is checked in both colour schemes, because a contrast
// rule can pass in one and fail in the other.
//
// Everything axe reports is a failure. There is no allowlist: an accepted
// violation is one nobody looks at again, and the /accessibility page says what
// is not checked rather than what is checked and ignored.
import { chromium } from "playwright";
import { AxeBuilder } from "@axe-core/playwright";
import { PAGES, BASE } from "./pages.mjs";

// WCAG 2.2 AA is the target, and best-practice rules are included because they
// are cheap and this site has no legacy markup to grandfather in.
const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa", "best-practice"];

const SCHEMES = ["light", "dark"];

const browser = await chromium.launch();
let violations = 0;
let checked = 0;

for (const scheme of SCHEMES) {
  const context = await browser.newContext({ colorScheme: scheme, viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();

  for (const { path, name, family } of PAGES) {
    // The kernel rate-limits every GET at 100 a minute per IP, static assets
    // included, and axe needs a fully rendered page. See docs/LEDGER.md, Gate 2.
    let response = await page.goto(BASE + path, { waitUntil: "networkidle" });
    for (let attempt = 0; attempt < 3 && response?.status() === 429; attempt++) {
      const wait = Number(response.headers()["retry-after"] ?? 60);
      console.log(`      429, waiting ${wait}s before retrying ${path}`);
      await new Promise((r) => setTimeout(r, (wait + 1) * 1000));
      response = await page.goto(BASE + path, { waitUntil: "networkidle" });
    }

    if (response?.status() !== 200) {
      console.error(`${String(response?.status())}  ${scheme}  ${path}`);
      violations++;
      continue;
    }

    const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
    checked++;

    if (results.violations.length === 0) {
      console.log(`ok    ${scheme.padEnd(5)} ${path}  (${family})`);
      continue;
    }

    console.error(`FAIL  ${scheme.padEnd(5)} ${path}  (${family})`);
    for (const v of results.violations) {
      violations++;
      console.error(`        ${v.id} [${v.impact}] ${v.help}`);
      for (const node of v.nodes.slice(0, 3)) {
        console.error(`          ${node.target.join(" ")}`);
        for (const line of (node.failureSummary ?? "").split("\n").slice(0, 3)) {
          if (line.trim()) console.error(`            ${line.trim()}`);
        }
      }
      if (v.nodes.length > 3) {
        console.error(`          …and ${v.nodes.length - 3} more`);
      }
    }
  }

  await context.close();
}

await browser.close();

console.log(`\n${checked} page renders checked against ${TAGS.join(", ")}`);
if (violations > 0) {
  console.error(`${violations} violation(s)`);
  process.exit(1);
}
console.log("no violations");

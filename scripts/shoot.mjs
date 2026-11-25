// Screenshot the page set at both widths in both color schemes.
//
//   npm run shoot
//
// Writes screenshots/{name}-{width}-{scheme}.png and prints one line per shot
// with the page's scroll width, so a horizontal overflow shows up as a number
// rather than as something to notice in an image.
import { chromium } from "playwright";
import { mkdir, writeFile } from "node:fs/promises";
import { PAGES, BASE, LAUNCH } from "./pages.mjs";

const WIDTHS = [
  { width: 360, height: 780 },
  { width: 1280, height: 900 },
];
const SCHEMES = ["light", "dark"];
const OUT = "screenshots";

const browser = await chromium.launch(LAUNCH);
await mkdir(OUT, { recursive: true });

const report = [];
let overflows = 0;
let unthemedTotal = 0;
let badStatus = 0;
const offOrigin = new Set();

for (const scheme of SCHEMES) {
  for (const viewport of WIDTHS) {
    const context = await browser.newContext({ viewport, colorScheme: scheme });
    const page = await context.newPage();

    // Nothing the site serves may pull from another origin. No font host, no
    // CDN, no analytics: a page that reaches off-site tells a third party who
    // read it, and the /accessibility and /search pages both make claims that
    // depend on this being true rather than intended.
    page.on("request", (request) => {
      const url = new URL(request.url());
      if (url.origin !== new URL(BASE).origin && url.protocol !== "data:") {
        offOrigin.add(`${url.origin} (${request.resourceType()})`);
      }
    });

    for (const { path, name, family } of PAGES) {
      // The kernel rate-limits every GET at 100 a minute per IP, static assets
      // included, and a screenshot pass is ten pages of real rendering in four
      // configurations. A 429 is answered the way the header asks rather than
      // counted as a broken page. See docs/LEDGER.md, Gate 2.
      //
      // That holds for the page's own requests too, not just the document: if
      // the stylesheet is the request that trips the limit, the page renders
      // unstyled and every control reads as unthemed. So a 429 on any request
      // the page made reloads the page, after the wait its header asks for.
      let limited = null;
      const onResponse = (r) => {
        if (r.status() === 429 && !limited) limited = r;
      };
      page.on("response", onResponse);
      const load = async () => {
        limited = null;
        const res = await page.goto(BASE + path, { waitUntil: "networkidle" });
        return { res, hit: res?.status() === 429 ? res : limited };
      };
      let { res: response, hit } = await load();
      for (let attempt = 0; attempt < 3 && hit; attempt++) {
        const wait = Number(hit.headers()["retry-after"] ?? 60);
        const what = hit.url() === response?.url() ? path : `${path} (${new URL(hit.url()).pathname} was limited)`;
        console.log(`      429, waiting ${wait}s before retrying ${what}`);
        await new Promise((r) => setTimeout(r, (wait + 1) * 1000));
        ({ res: response, hit } = await load());
      }
      page.off("response", onResponse);
      // Still limited after the retries: the page did not render as served, so
      // it is not measured as though it had.
      if (hit && response?.status() === 200) {
        console.log(`      ${new URL(hit.url()).pathname} still answered 429 on ${path}`);
        response = hit;
      }
      const status = response?.status() ?? 0;

      // The front page plays a short entrance (site.css, the Motion section);
      // every end state is the page at rest. Wait it out so the archived
      // screenshot and the element measurements see the settled page.
      await page.waitForTimeout(1800);

      // A page that did not render is not a page to measure. Repeated runs
      // against a local stack trip the login rate limiter, and a 429's error
      // page would otherwise be counted as the site's own unthemed markup.
      if (status !== 200) {
        badStatus++;
        console.log(`${scheme.padEnd(5)} ${String(viewport.width).padStart(4)}  ${status}  SKIPPED   ${path}`);
        report.push({ path, name, family, scheme, width: viewport.width, status, skipped: true });
        continue;
      }

      // The body must never scroll sideways. A wide table or a long code block
      // scrolls inside its own box; the document does not.
      //
      // The second half of this looks for elements the site forgot to theme.
      // A kernel or plugin template the site did not override can leave a
      // control at its browser default — a white submit button on a dark page —
      // and that is invisible in a report that only samples the body. So every
      // element is measured, and in dark mode a light background is a failure.
      const overflow = await page.evaluate((scheme) => {
        // Two notations, because the channels mean different numbers in each.
        // `rgb(250, 245, 239)` counts 0 to 255; `color(srgb 0.98 0.96 0.94)`,
        // which is how a browser serializes the `color-mix()` the sticky header
        // uses, counts 0 to 1. Reading the second as though it were the first
        // divides by 255 twice and reports a near-white plate as near-black, so
        // the scale is chosen by the notation rather than assumed.
        const luminance = (rgb) => {
          const m = rgb.match(/\d+(\.\d+)?/g);
          if (!m || m.length < 3) return null;
          // A fully transparent background is not a background.
          if (m.length > 3 && Number(m[3]) === 0) return null;
          const scale = rgb.startsWith("color(") ? 1 : 255;
          const ch = (v) => {
            const c = Number(v) / scale;
            return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
          };
          return 0.2126 * ch(m[0]) + 0.7152 * ch(m[1]) + 0.0722 * ch(m[2]);
        };

        const unthemed = [];
        for (const el of document.querySelectorAll("body *")) {
          if (el.offsetParent === null && el.tagName !== "BODY") continue;
          const cs = getComputedStyle(el);
          const bg = luminance(cs.backgroundColor);
          if (bg === null) continue;
          // In dark mode nothing should sit on a near-white plate; in light mode
          // nothing should sit on a near-black one.
          const wrong = scheme === "dark" ? bg > 0.5 : bg < 0.12;
          if (wrong) {
            unthemed.push(
              `${el.tagName.toLowerCase()}${el.className && typeof el.className === "string" ? "." + el.className.trim().split(/\s+/).join(".") : ""} bg=${cs.backgroundColor}`
            );
          }
        }

        return {
          scrollWidth: document.documentElement.scrollWidth,
          clientWidth: document.documentElement.clientWidth,
          bodyBg: getComputedStyle(document.body).backgroundColor,
          bodyColor: getComputedStyle(document.body).color,
          font: getComputedStyle(document.body).fontFamily,
          unthemed: [...new Set(unthemed)],
        };
      }, scheme);
      const overflowing = overflow.scrollWidth > overflow.clientWidth + 1;
      if (overflowing) overflows++;
      if (overflow.unthemed.length) {
        unthemedTotal += overflow.unthemed.length;
        for (const u of overflow.unthemed) {
          console.log(`      UNTHEMED ${scheme} ${path}: ${u}`);
        }
      }

      const file = `${OUT}/${name}-${viewport.width}-${scheme}.png`;
      await page.screenshot({ path: file, fullPage: true });

      report.push({ path, name, family, scheme, width: viewport.width, status, overflowing, ...overflow });
      console.log(
        `${scheme.padEnd(5)} ${String(viewport.width).padStart(4)}  ${String(status)}  ` +
        `${overflowing ? "OVERFLOW" : "        "}  bg=${overflow.bodyBg}  ${path}`
      );
    }
    await context.close();
  }
}

await browser.close();
await writeFile(`${OUT}/report.json`, JSON.stringify(report, null, 2));

let failed = false;
if (overflows > 0) {
  console.error(`\n${overflows} page/viewport combinations scroll horizontally`);
  failed = true;
}
if (badStatus > 0) {
  console.error(`${badStatus} page/viewport combinations did not return 200`);
  failed = true;
}
if (offOrigin.size > 0) {
  console.error(`the site requested ${offOrigin.size} off-origin resource(s):`);
  for (const o of offOrigin) console.error(`  ${o}`);
  failed = true;
}
if (unthemedTotal > 0) {
  console.error(`${unthemedTotal} elements carry a background from the wrong scheme`);
  failed = true;
}
if (failed) process.exit(1);
console.log(`\n${report.length} screenshots, no horizontal overflow, nothing unthemed, no off-origin requests`);

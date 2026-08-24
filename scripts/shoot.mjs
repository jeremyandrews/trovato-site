// Screenshot the page set at both widths in both color schemes.
//
//   npm run shoot
//
// Writes screenshots/{name}-{width}-{scheme}.png and prints one line per shot
// with the page's scroll width, so a horizontal overflow shows up as a number
// rather than as something to notice in an image.
import { chromium } from "playwright";
import { mkdir, writeFile } from "node:fs/promises";
import { PAGES, BASE } from "./pages.mjs";

const WIDTHS = [
  { width: 360, height: 780 },
  { width: 1280, height: 900 },
];
const SCHEMES = ["light", "dark"];
const OUT = "screenshots";

const browser = await chromium.launch();
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
      const response = await page.goto(BASE + path, { waitUntil: "networkidle" });
      const status = response?.status() ?? 0;

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
        const luminance = (rgb) => {
          const m = rgb.match(/\d+(\.\d+)?/g);
          if (!m || m.length < 3) return null;
          // A fully transparent background is not a background.
          if (m.length > 3 && Number(m[3]) === 0) return null;
          const ch = (v) => {
            const c = Number(v) / 255;
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

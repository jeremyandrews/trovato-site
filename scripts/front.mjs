// The front page, in English and Italian, prints each of its parts once and
// none of its own source.
//
//   npm run front
//
// Fetches `/` and `/it/` from SITE_BASE (default http://127.0.0.1:3080) and
// applies the rules in scripts/front-check.mjs. Plain HTTP, no browser: what is
// checked is what the server sent.
//
// Redirects are not followed. Each address has to answer 200 itself, so a
// redirect from `/it/` to `/` is a failure and not the English page checked
// twice. The two ledes must also differ: the Italian page is a translation.
import { BASE } from "./pages.mjs";
import { frontPageProblems, ledeOf } from "./front-check.mjs";
import { retryAfterMs } from "./ratelimit.mjs";

async function get(url) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const response = await fetch(url, { redirect: "manual" });
    if (response.status !== 429) return response;
    const wait = retryAfterMs(response.headers.get("retry-after"));
    console.log(`      429, waiting ${Math.ceil(wait / 1000)}s before retrying ${url}`);
    await new Promise((r) => setTimeout(r, wait + 1000));
  }
  return fetch(url, { redirect: "manual" });
}

let failed = 0;
const ledes = {};
for (const [path, lang] of [["/", "en"], ["/it/", "it"]]) {
  const response = await get(BASE + path);
  let problems;
  if (response.status === 200) {
    const html = await response.text();
    ledes[lang] = ledeOf(html);
    problems = frontPageProblems(html, lang);
  } else {
    const to = response.headers.get("location");
    problems = [`status ${response.status}${to ? ` to ${to}` : ""}`];
  }
  if (problems.length === 0) {
    console.log(`ok    ${path}`);
  } else {
    failed++;
    console.log(`FAIL  ${path}`);
    for (const p of problems) console.log(`      ${p}`);
  }
}
if (ledes.en && ledes.en === ledes.it) {
  failed++;
  console.log(`FAIL  /it/ has the English lede "${ledes.en}"`);
}
if (failed) process.exit(1);

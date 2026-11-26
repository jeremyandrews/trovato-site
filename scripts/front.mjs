// The front page, in English and Italian, prints each of its parts once and
// none of its own source.
//
//   npm run front
//
// Fetches `/` and `/it/` from SITE_BASE (default http://127.0.0.1:3080) and
// applies the rules in scripts/front-check.mjs. Plain HTTP, no browser: what is
// checked is what the server sent.
import { BASE } from "./pages.mjs";
import { frontPageProblems } from "./front-check.mjs";
import { retryAfterMs } from "./ratelimit.mjs";

async function get(url) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const response = await fetch(url);
    if (response.status !== 429) return response;
    const wait = retryAfterMs(response.headers.get("retry-after"));
    console.log(`      429, waiting ${Math.ceil(wait / 1000)}s before retrying ${url}`);
    await new Promise((r) => setTimeout(r, wait + 1000));
  }
  return fetch(url);
}

let failed = 0;
for (const path of ["/", "/it/"]) {
  const response = await get(BASE + path);
  const problems =
    response.status === 200
      ? frontPageProblems(await response.text())
      : [`status ${response.status}`];
  if (problems.length === 0) {
    console.log(`ok    ${path}`);
  } else {
    failed++;
    console.log(`FAIL  ${path}`);
    for (const p of problems) console.log(`      ${p}`);
  }
}
if (failed) process.exit(1);

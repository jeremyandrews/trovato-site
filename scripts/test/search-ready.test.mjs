// The browser gates against a fixture site whose search page renders only the
// server results, as it does when the in-browser index fails to load. Both
// gates must fail, and for that reason alone: every other page is clean, so a
// pass here would mean the gates had checked the fallback and called it the
// search page.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const scripts = join(dirname(fileURLToPath(import.meta.url)), "..");

const PAGE = `<!doctype html><html lang="en"><head><title>fixture</title></head>
<body><main><h1>Search</h1>
<div id="search-fallback"><p>Server results only.</p></div>
</main></body></html>`;

async function run(script) {
  const server = createServer((req, res) => {
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(PAGE);
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const base = `http://127.0.0.1:${server.address().port}`;
  const cwd = await mkdtemp(join(tmpdir(), "search-ready-test-"));
  try {
    return await new Promise((resolve) => {
      const child = spawn(process.execPath, [join(scripts, script)], {
        cwd,
        env: { ...process.env, SITE_BASE: base },
      });
      let out = "";
      child.stdout.on("data", (d) => (out += d));
      child.stderr.on("data", (d) => (out += d));
      child.on("close", (code) => resolve({ code, out }));
    });
  } finally {
    server.close();
    await rm(cwd, { recursive: true, force: true });
  }
}

const MISSING = /NOT READY.*\/search\?q=Drupal: missing \.scolta-result-card, #search-fallback\[hidden\]/;

test("shoot fails a search page that shows only the server results", { timeout: 600_000 }, async () => {
  const { code, out } = await run("shoot.mjs");
  assert.equal(code, 1, out);
  assert.match(out, MISSING, out);
  assert.match(out, /4 page\/viewport combinations did not render what they are listed for/, out);
  assert.doesNotMatch(out, /OVERFLOW|UNTHEMED|did not return 200|off-origin/, out);
});

test("a11y fails a search page that shows only the server results", { timeout: 600_000 }, async () => {
  const { code, out } = await run("a11y.mjs");
  assert.equal(code, 1, out);
  assert.match(out, MISSING, out);
  assert.match(out, /\n2 violation\(s\)/, out);
  assert.doesNotMatch(out, /^FAIL /m, out);
});

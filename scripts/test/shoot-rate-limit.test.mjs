// shoot.mjs against a fixture server whose page asks for an image a second
// after it loads, inside the settle before the measurement and the capture.
// The first request for that image is rate limited, with Retry-After given as
// an HTTP-date. The run must notice the 429, wait until that date, and retake
// the page, rather than archive a screenshot with the image missing.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const shoot = join(dirname(fileURLToPath(import.meta.url)), "..", "shoot.mjs");

const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=",
  "base64",
);
const PAGE = `<!doctype html><html lang="en"><head><title>fixture</title></head>
<body><main><p>A page whose image arrives late.</p>
<div class="scolta-result-card">A result</div><div id="search-fallback" hidden></div></main>
<script>setTimeout(() => { const i = new Image(1, 1); i.src = "/late.png?" + Date.now(); document.body.append(i); }, 1000);</script>
</body></html>`;

test("a 429 on a request made after the page settles retakes the page, after the wait the header asks for", { timeout: 600_000 }, async () => {
  let limitedAt = null;
  let retryAfter = null;
  let firstPathAfterLimit = null;
  const server = createServer((req, res) => {
    const url = new URL(req.url, "http://fixture");
    if (url.pathname === "/late.png") {
      if (limitedAt === null) {
        limitedAt = Date.now();
        retryAfter = new Date(limitedAt + 3000).toUTCString();
        res.writeHead(429, { "retry-after": retryAfter, "content-type": "text/plain" });
        return res.end("slow down");
      }
      res.writeHead(200, { "content-type": "image/png" });
      return res.end(PNG);
    }
    if (limitedAt !== null && firstPathAfterLimit === null && req.headers.accept?.includes("text/html")) {
      firstPathAfterLimit = Date.now();
    }
    res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    res.end(PAGE);
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const base = `http://127.0.0.1:${server.address().port}`;
  const cwd = await mkdtemp(join(tmpdir(), "shoot-test-"));
  try {
    const { code, out } = await new Promise((resolve) => {
      const child = spawn(process.execPath, [shoot], { cwd, env: { ...process.env, SITE_BASE: base } });
      let out = "";
      child.stdout.on("data", (d) => (out += d));
      child.stderr.on("data", (d) => (out += d));
      child.on("close", (code) => resolve({ code, out }));
    });
    assert.equal(code, 0, out);
    assert.match(out, /429, waiting \d+s before retrying \S+ \(\/late\.png was limited\)/, out);
    assert.ok(limitedAt !== null, "the fixture never saw the late image requested");
    // The retry waits until the date in the header, not a moment later.
    assert.ok(
      firstPathAfterLimit - limitedAt >= Date.parse(retryAfter) - limitedAt,
      `retried ${firstPathAfterLimit - limitedAt}ms after the 429; the header asked to wait until ${retryAfter}`,
    );
  } finally {
    server.close();
    await rm(cwd, { recursive: true, force: true });
  }
});

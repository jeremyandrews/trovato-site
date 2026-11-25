import { test } from "node:test";
import assert from "node:assert/strict";
import { retryAfterMs } from "../ratelimit.mjs";

const now = Date.parse("2026-11-25T12:00:00Z");

test("delay seconds are read as seconds", () => {
  assert.equal(retryAfterMs("30", now), 30_000);
  assert.equal(retryAfterMs(" 0 ", now), 0);
});

test("an HTTP-date waits until that date", () => {
  assert.equal(retryAfterMs("Wed, 25 Nov 2026 12:00:30 GMT", now), 30_000);
});

test("an HTTP-date in the past means no wait", () => {
  assert.equal(retryAfterMs("Wed, 25 Nov 2026 11:59:00 GMT", now), 0);
});

test("a missing or unreadable header falls back", () => {
  assert.equal(retryAfterMs(undefined, now), 60_000);
  assert.equal(retryAfterMs("", now), 60_000);
  assert.equal(retryAfterMs("soon", now), 60_000);
  assert.equal(retryAfterMs("-5", now, 10), 10_000);
});

// How long a 429 asks the client to wait, in milliseconds.
//
// Retry-After is either a number of seconds or an HTTP-date (RFC 9110,
// section 10.2.3). Reading a date with Number() gives NaN, and a timeout of
// NaN fires at once, so a date-valued header would retry immediately and
// spend every attempt before the limit lifts. A date in the past means no
// wait; a missing or unreadable header falls back to `fallbackSeconds`.
export function retryAfterMs(value, now = Date.now(), fallbackSeconds = 60) {
  const text = String(value ?? "").trim();
  if (/^\d+$/.test(text)) return Number(text) * 1000;
  // An HTTP-date always names its day and zone in letters; requiring one keeps
  // Date.parse from reading something like "-5" as the year 5 BC.
  const date = /[A-Za-z]/.test(text) ? Date.parse(text) : NaN;
  if (!Number.isNaN(date)) return Math.max(0, date - now);
  return fallbackSeconds * 1000;
}

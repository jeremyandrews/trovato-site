#!/usr/bin/env bash
# Every gate CI runs, locally, in CI's order, stopping at the first failure.
#
#   ./scripts/check-all.sh
#
# The order is not cosmetic. It is CI's order, so the first thing that fails here
# is the first thing that would have failed there, and the cheap checks come
# first: there is no reason to spend four minutes crawling a site whose code does
# not format.
#
# This exists because the README's list was not CI's list. `cargo fmt --all
# --check` and `cargo clippy --all-targets -- -D warnings` ran in CI and appeared
# nowhere local, so on 2026-09-19 a merge that passed every documented gate failed
# on push the moment the toolchain moved under it. A list that does not predict CI
# is worse than no list, and a list nobody can run in one command drifts back out
# of step, which is why this is a script and not a longer README.
#
# The browser gates need the site up. Bring it up first:
#
#   ./scripts/run.sh --fresh --proxy
#
# and this script says so rather than guessing, because starting a site is a
# side effect and a checker should not have those.
set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

BASE="${SITE_BASE:-http://127.0.0.1:3080}"

failed=""
ran=0

run() {
  local label="$1"
  shift
  ran=$((ran + 1))
  printf '\n\033[1m=== %s ===\033[0m\n%s\n\n' "$label" "$*"
  if "$@"; then
    printf '\n\033[32mPASS\033[0m  %s\n' "$label"
  else
    local code=$?
    printf '\n\033[31mFAIL\033[0m  %s (exit %d)\n' "$label" "$code"
    failed="$label"
    return 1
  fi
}

# The Rust gates need nothing running.
run "cargo fmt"      cargo fmt --all --check                          || exit 1
run "cargo clippy"   cargo clippy --all-targets -- -D warnings        || exit 1
run "cargo test"     cargo test --all                                 || exit 1
run "docs mirror"    cargo run -p tools --bin docs-import -- --check  || exit 1

# Everything below drives a real browser against a real site. Say so plainly
# rather than failing six gates in a row with a connection error.
if ! curl -fsS -o /dev/null --max-time 5 "$BASE/"; then
  printf '\n\033[31mFAIL\033[0m  the site is not answering on %s\n' "$BASE"
  printf '      bring it up first:  ./scripts/run.sh --fresh --proxy\n'
  printf '\n%d gate(s) passed, then stopped.\n' "$ran"
  exit 1
fi

run "crawl"          npm run crawl                    || exit 1
run "a11y"           npm run a11y                     || exit 1
run "shoot"          npm run shoot                    || exit 1
run "moderation"     ./scripts/check-moderation.sh    || exit 1
run "config round-trip" ./scripts/check-roundtrip.sh  || exit 1
run "production profile" ./scripts/check-production.sh || exit 1

printf '\n\033[32mAll %d gates passed.\033[0m\n' "$ran"

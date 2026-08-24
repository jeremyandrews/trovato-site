#!/usr/bin/env bash
# Export the running site's config and check it against what this repository
# declares.
#
#   ./scripts/check-roundtrip.sh
#
# The gate behind "the site definition lives in git": every key in config/ must
# come back from `trovato config export` with the same value. An export carries
# more than the site declares — stages, roles and content types the kernel and
# its plugins create — so this is a one-way comparison, not a diff.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

PROJECT="${COMPOSE_PROJECT_NAME:-trovato-site}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "==> Exporting the running site's config"
docker compose -p "$PROJECT" exec -T site sh -c \
    'rm -rf /tmp/config-export && mkdir -p /tmp/config-export && ./trovato config export /tmp/config-export' \
    | tail -n +1
docker compose -p "$PROJECT" cp "site:/tmp/config-export/." "$work/"

echo "==> Comparing against config/"
cargo run -q -p checks --bin config-roundtrip -- config "$work"

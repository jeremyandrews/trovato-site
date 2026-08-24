#!/usr/bin/env bash
# Bring up the whole site, from nothing to a populated, themed, working site.
#
#   ./scripts/run.sh          bring it up (idempotent — safe to re-run)
#   ./scripts/run.sh --fresh  destroy this project's volumes first, then bring it up
#
# Everything it touches is scoped to the compose project named below. It never
# runs a global docker prune and never removes anything it did not create.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

PROJECT="${COMPOSE_PROJECT_NAME:-trovato-site}"
PORT="${SITE_PORT:-3080}"
BASE="http://127.0.0.1:${PORT}"

# The local administrator. Local only: the production profile takes its
# credentials from the environment and never from a file in the repository.
ADMIN_USER="${ADMIN_USER:-admin}"
ADMIN_MAIL="${ADMIN_MAIL:-admin@trovato.rs}"
ADMIN_PASS="${ADMIN_PASS:-trovato-local-admin}"
SITE_NAME="${SITE_NAME:-Trovato}"
SITE_SLOGAN="${SITE_SLOGAN:-A content management system written in Rust.}"

# The plugins the site turns on. Every one of them already ships in the kernel
# image; the site enables what exists rather than rebuilding it. `trovato_site`
# is the only one this repository builds.
SITE_PLUGINS=(
    trovato_site
)
IMAGE_PLUGINS=(
    trovato_blog
    trovato_seo
    trovato_search
    trovato_contact
    trovato_scheduled_publishing
    trovato_redirects
    trovato_comments
    # AI comment moderation. Enabled and inert: it queues a classification job
    # for every comment and the queue worker calls whichever AI provider the site
    # is configured with. This site has none, so the job dead-letters and the
    # comment stays held for a person. That is the designed failure direction and
    # there is a test for it.
    trovato_spam
)

dc() { docker compose -p "$PROJECT" "$@"; }
in_site() { dc exec -T site "$@"; }

# Wait for the site to answer its health check, or give up loudly.
wait_for_site() {
    for _ in $(seq 1 60); do
        if curl -fsS -o /dev/null "${BASE}/health" 2>/dev/null; then return 0; fi
        sleep 2
    done
    echo "the site never became healthy"
    dc logs --tail=50 site
    return 1
}

if [ "${1:-}" = "--fresh" ]; then
    echo "==> Removing this project's containers and volumes"
    dc down -v --remove-orphans
fi

./scripts/build-overlay.sh

echo "==> Starting the stack"
dc up -d --wait site

echo "==> Waiting for the site to answer"
wait_for_site

echo "==> Installing plugins"
for name in "${IMAGE_PLUGINS[@]}" "${SITE_PLUGINS[@]}"; do
    in_site ./trovato plugin install "$name" >/dev/null 2>&1 \
        || in_site ./trovato plugin enable "$name" >/dev/null 2>&1 \
        || echo "    WARNING: could not enable ${name}"
    echo "    ${name}"
done

# A plugin's content types are registered when the kernel loads it, so the
# server has to come back up after the install before config import can
# reference `news` or `front_page`.
echo "==> Restarting the site so the new plugins are loaded"
dc restart site >/dev/null
wait_for_site

echo "==> Running the installer"
cookies="$(mktemp)"
trap 'rm -f "$cookies"' EXIT
curl -fsS -c "$cookies" -o /dev/null -X POST "${BASE}/install/admin" \
    --data-urlencode "username=${ADMIN_USER}" \
    --data-urlencode "email=${ADMIN_MAIL}" \
    --data-urlencode "password=${ADMIN_PASS}" \
    --data-urlencode "password_confirm=${ADMIN_PASS}" >/dev/null 2>&1 \
    || echo "    (already installed)"
curl -fsS -b "$cookies" -o /dev/null -X POST "${BASE}/install/site" \
    --data-urlencode "site_name=${SITE_NAME}" \
    --data-urlencode "site_slogan=${SITE_SLOGAN}" \
    --data-urlencode "site_mail=${ADMIN_MAIL}" >/dev/null 2>&1 \
    || echo "    (already installed)"

echo "==> Importing the site config"
in_site ./trovato config import /site/config

# `config import` reads one directory and does not recurse, so the generated
# documentation set is its own import. Same precedent as the kernel tutorial's
# seed-italian/ directory, and it keeps 36 machine-written files out of the
# directory a person edits by hand.
echo "==> Importing the mirrored documentation"
in_site ./trovato config import /site/config/docs

# Gather queries and menu links are read into their registries when the kernel
# starts, so a query imported into a running server is in the database but not
# yet routable: /news answers `{"error":"query not found"}` and the main menu
# renders empty until the server reads them again. Hence the second restart.
# It is the reason `run.sh` exists rather than a line of documentation telling
# somebody to remember this.
echo "==> Restarting the site so the imported config is live"
dc restart site >/dev/null
wait_for_site

echo
echo "The site is on ${BASE}"
echo "Administrator: ${ADMIN_USER} / ${ADMIN_PASS}"

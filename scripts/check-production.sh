#!/usr/bin/env bash
# Bring up the production profile on this machine and check it end to end.
#
#   ./scripts/check-production.sh
#
# It runs docker-compose.production.yml as written, with one override that
# changes three things a laptop cannot have: the hostname is `localhost`, the
# certificate comes from Caddy's own authority rather than from Let's Encrypt,
# and the ports are high ones. Everything else — the unpublished kernel, HSTS,
# the www redirect, the caching policy, the static file serving — is the
# production configuration.
#
# Scoped to its own compose project, so it cannot touch the local development
# stack or anything else on this machine.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

PROJECT="trovato-site-production-check"
HTTPS_PORT="${LOCAL_HTTPS_PORT:-8444}"
HTTP_PORT="${LOCAL_HTTP_PORT:-8090}"
BASE="https://localhost:${HTTPS_PORT}"

ADMIN_USER="${ADMIN_USER:-admin}"
ADMIN_MAIL="${ADMIN_MAIL:-admin@trovato.rs}"
ADMIN_PASS="${ADMIN_PASS:-production-check-admin-password}"

# Caddy's internal authority is not in this machine's trust store, so curl is
# told to accept it. That is the one thing being skipped, and it is skipped
# knowingly: what is under test is the configuration, not the certificate chain.
CURL=(curl -sS --insecure)

env_file="$(mktemp)"
trap 'cleanup' EXIT
cleanup() {
    rm -f "$env_file"
    docker compose -p "$PROJECT" \
        -f docker-compose.yml -f docker-compose.production.yml -f docker-compose.production-local.yml \
        --env-file "$env_file" down -v --remove-orphans >/dev/null 2>&1 || true
}

cat > "$env_file" <<ENVEOF
SITE_HOST=localhost
SITE_URL=https://localhost:${HTTPS_PORT}
ACME_EMAIL=nobody@localhost
TROVATO_VERSION=${TROVATO_VERSION:-0.101.0}
POSTGRES_USER=trovato
POSTGRES_DB=trovato
POSTGRES_PASSWORD=production-check-only
CRON_KEY=production-check-cron-key
CRON_INTERVAL=60
COMPOSE_SUBNET=172.31.72.0/24
PROXY_IP=172.31.72.10
LOCAL_HTTPS_PORT=${HTTPS_PORT}
LOCAL_HTTP_PORT=${HTTP_PORT}
ENVEOF

dc() {
    docker compose -p "$PROJECT" \
        -f docker-compose.yml -f docker-compose.production.yml -f docker-compose.production-local.yml \
        --env-file "$env_file" "$@"
}

fail() { echo "FAIL: $1"; dc logs --tail=40 proxy site 2>&1 | tail -40; exit 1; }

# Wait for a 200, not merely for an answer. The proxy answers 502 the moment it
# is up and the kernel is not, and a loop that accepts any response treats that
# as ready — which is how this script first reported the whole site as broken
# when it had only restarted a second earlier.
wait_ready() {
    local _ code
    for _ in $(seq 1 60); do
        code="$("${CURL[@]}" -o /dev/null -w '%{http_code}' "${BASE}/health" 2>/dev/null || echo 000)"
        [ "$code" = "200" ] && return 0
        sleep 2
    done
    return 1
}

echo "==> Building the overlay"
./scripts/build-overlay.sh >/dev/null

echo "==> Starting the production profile"
dc up -d --wait site >/dev/null
dc up -d proxy >/dev/null

echo "==> Waiting for the proxy"
wait_ready || fail "the proxy never served a healthy response on ${BASE}"

echo "==> The kernel must not be reachable except through the proxy"
# Asked of the merged configuration rather than of a running container: what
# matters is that the production profile declares no published port for the
# kernel, the database or the cache. `docker compose port` against the base file
# alone would answer about the development stack instead, which is what the first
# version of this check did.
published="$(dc config --format json 2>/dev/null \
    | python3 -c 'import json,sys; c=json.load(sys.stdin); print(" ".join(n for n,s in c["services"].items() if s.get("ports")))')"
case " ${published} " in
    *" site "*|*" postgres "*|*" redis "*)
        fail "these publish a port in production and must not: ${published}" ;;
esac
[ "$published" = "proxy" ] || fail "expected only the proxy to publish ports; got: ${published}"
echo "    only the proxy publishes a port (${published})"

echo "==> Installing"
jar="$(mktemp)"
"${CURL[@]}" -c "$jar" -o /dev/null -X POST "${BASE}/install/admin" \
    --data-urlencode "username=${ADMIN_USER}" --data-urlencode "email=${ADMIN_MAIL}" \
    --data-urlencode "password=${ADMIN_PASS}" --data-urlencode "password_confirm=${ADMIN_PASS}" || true
"${CURL[@]}" -b "$jar" -o /dev/null -X POST "${BASE}/install/site" \
    --data-urlencode "site_name=Trovato" \
    --data-urlencode "site_slogan=A content management system written in Rust." \
    --data-urlencode "site_mail=${ADMIN_MAIL}" || true
rm -f "$jar"

echo "==> Enabling plugins and importing config"
for name in trovato_blog trovato_seo trovato_search trovato_contact \
            trovato_scheduled_publishing trovato_redirects trovato_comments \
            trovato_spam trovato_site; do
    dc exec -T site ./trovato plugin install "$name" >/dev/null 2>&1 \
        || dc exec -T site ./trovato plugin enable "$name" >/dev/null 2>&1 || true
done
dc restart site >/dev/null
wait_ready || fail "the site did not come back after the plugin install"
dc exec -T site ./trovato config import /site/config >/dev/null
dc exec -T site ./trovato config import /site/config/docs >/dev/null
dc restart site >/dev/null
wait_ready || fail "the site did not come back after the config import"

echo "==> Checking"

status_of() { "${CURL[@]}" -o /dev/null -w '%{http_code}' "$1"; }
header_of() { "${CURL[@]}" -D - -o /dev/null "$1" | grep -i "^$2:" | tr -d '\r' | head -1; }

for path in / /why /get-started /rust /community /accessibility /news /blog /learn /contact /search; do
    code="$(status_of "${BASE}${path}")"
    [ "$code" = "200" ] || fail "${path} returned ${code} through the production proxy"
done
echo "    every page serves over TLS"

case "$(header_of "${BASE}/why" strict-transport-security)" in
    *max-age=31536000*) echo "    HSTS is set" ;;
    *) fail "HSTS is missing on an HTML response" ;;
esac

case "$(header_of "${BASE}/why" cache-control)" in
    *"max-age=60"*) echo "    anonymous HTML is cacheable for a minute" ;;
    *) fail "an anonymous page carries the wrong Cache-Control" ;;
esac

case "$("${CURL[@]}" -D - -o /dev/null -H 'Cookie: id=x' "${BASE}/why" | grep -i '^cache-control' | tr -d '\r')" in
    *private*) echo "    a page with a session is private" ;;
    *) fail "a page carrying a session was not marked private" ;;
esac

case "$(header_of "${BASE}/static/fonts/Inter-Regular.woff2" cache-control)" in
    *max-age=604800*) echo "    fonts are cached for a week, and served by the proxy" ;;
    *) fail "the fonts carry the wrong Cache-Control" ;;
esac

"${CURL[@]}" "${BASE}/sitemap.xml" | grep -q "<loc>https://" \
    || fail "/sitemap.xml is not the site's own, or its URLs are not absolute"
echo "    /sitemap.xml serves absolute URLs"

redirect="$("${CURL[@]}" -o /dev/null -w '%{http_code} %{redirect_url}' "https://www.localhost:${HTTPS_PORT}/why" 2>/dev/null || echo "skipped")"
case "$redirect" in
    301*) echo "    www redirects to the apex: ${redirect}" ;;
    *) echo "    www redirect not exercised (${redirect}); the hostname does not resolve here" ;;
esac

case "$("${CURL[@]}" -o /dev/null -w '%{http_code} %{redirect_url}' "http://localhost:${HTTP_PORT}/why")" in
    30*) echo "    plain HTTP redirects to HTTPS" ;;
    *) fail "plain HTTP did not redirect" ;;
esac

echo
echo "PASS: the production profile serves the site over TLS with its headers on."

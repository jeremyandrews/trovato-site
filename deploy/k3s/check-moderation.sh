#!/usr/bin/env bash
# scripts/check-moderation.sh against a throwaway site in k3s.
#
#   deploy/k3s/check-moderation.sh
#
# The check registers an account and posts a comment, so it never runs against
# the review site. This brings up modcheck.yaml (PostgreSQL, Redis and the
# kernel in one pod on emptyDir, running the image the review site runs),
# populates it the way apply.sh populates the review site, reaches it through
# kubectl port-forward, runs the unmodified script with shim/docker standing in
# for `docker compose exec`, and deletes everything it created on the way out,
# pass or fail.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
. "$here/lib.sh"

LOCAL_PORT="${MODCHECK_PORT:-38080}"
work="$(mktemp -d "${TMPDIR:-/tmp}/trovato-modcheck.XXXXXX")"
pf_pid=""

cleanup() {
    [ -n "$pf_pid" ] && kill "$pf_pid" 2>/dev/null || true
    echo "==> Deleting the throwaway site"
    k delete deployment,secret -l app=trovato-modcheck --ignore-not-found --wait=true >/dev/null
    k wait --for=delete pod -l app=trovato-modcheck --timeout=120s >/dev/null 2>&1 || true
    rm -rf "$work"
}
# Until this run owns the throwaway site, leaving removes only its own scratch
# directory. The label-selected cleanup is armed after the secret below has
# been created, and creating it is what fails if another run already holds it,
# so a second run started alongside this one never deletes the first one's site.
trap 'rm -rf "$work"' EXIT

SITE_IMAGE="$(k get deployment trovato-site -o jsonpath='{.spec.template.spec.containers[?(@.name=="site")].image}')"
[ -n "$SITE_IMAGE" ] || { echo "the review site is not deployed; run apply.sh first" >&2; exit 1; }

echo "==> Bringing up a throwaway site on ${SITE_IMAGE}"
CRON_KEY="$(openssl rand -hex 32)"
ADMIN_PASS="$(openssl rand -hex 16)"
k create secret generic trovato-modcheck-secrets \
    --from-literal=POSTGRES_PASSWORD="$(openssl rand -hex 24)" \
    --from-literal=CRON_KEY="$CRON_KEY" >/dev/null \
    || { echo "trovato-modcheck-secrets already exists: another moderation check is running, or one did not clean up" >&2; exit 1; }
trap cleanup EXIT
k label secret trovato-modcheck-secrets app=trovato-modcheck >/dev/null
sed "s|__SITE_IMAGE__|${SITE_IMAGE}|g" "$here/modcheck.yaml" | kubectl apply -f - >/dev/null
k rollout status deployment/trovato-modcheck --timeout=300s

SITE_SELECTOR="app=trovato-modcheck"
SITE_CONTAINER="site"
# Not a rollout restart: that would make a new pod and lose the emptyDir
# database. SIGTERM to the kernel shuts it down cleanly and the kubelet starts
# the container again inside the same pod.
restart_container() {
    in_site sh -c 'kill -TERM 1' || true
    sleep 3
    wait_for_kernel
}
RESTART=restart_container
populate

echo "==> Port-forwarding 127.0.0.1:${LOCAL_PORT} to the throwaway kernel"
pod="$(pod_for "$SITE_SELECTOR")"
k port-forward "pod/${pod}" "${LOCAL_PORT}:3000" >"$work/port-forward.log" 2>&1 &
pf_pid=$!
disown "$pf_pid"
for _ in $(seq 1 30); do
    curl -fsS -o /dev/null "http://127.0.0.1:${LOCAL_PORT}/health" 2>/dev/null && break
    sleep 1
done

echo "==> Running scripts/check-moderation.sh"
export PATH="$here/shim:$PATH"
export K3S_NS="$NS"
export K3S_SITE_SELECTOR="$SITE_SELECTOR" K3S_SITE_CONTAINER=site
export K3S_POSTGRES_SELECTOR="$SITE_SELECTOR" K3S_POSTGRES_CONTAINER=postgres
export SITE_PORT="$LOCAL_PORT" CRON_KEY
"$here/../../scripts/check-moderation.sh"

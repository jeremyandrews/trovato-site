#!/usr/bin/env bash
# Build, push, deploy and populate the trovato.rs review site on the home k3s
# cluster, from whatever is in this working tree right now.
#
#   deploy/k3s/apply.sh                 build, push, diff, apply, populate
#   deploy/k3s/apply.sh --no-populate   stop after the rollout
#
# Idempotent. Re-running it after a content change builds a new image (the tag
# carries a hash of the content when the tree is dirty), rolls the site onto
# it, and imports the config again, which overwrites what is in the database.
#
# Environment: KUBECONFIG (default ~/.kube/ks1.yaml), LB_IP (default
# 10.20.250.17), SITE_HOST (default <LB_IP>.nip.io, the host SITE_URL names),
# IMAGE_REPO (default registry.pozza/jeremyandrews/trovato-site), PUSH_HOST
# (default ks1.pozza, used only when this machine has no push login).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
cd "$repo_root"
# shellcheck source=lib.sh
. "$here/lib.sh"

POPULATE=1
for arg in "$@"; do
    case "$arg" in
        --no-populate) POPULATE=0 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

LB_IP="${LB_IP:-10.20.250.17}"
SITE_HOST="${SITE_HOST:-${LB_IP}.nip.io}"
IMAGE_REPO="${IMAGE_REPO:-registry.pozza/jeremyandrews/trovato-site}"
KERNEL_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' kernel-release.toml)"
KERNEL_IMAGE="ghcr.io/jeremyandrews/trovato:${KERNEL_VERSION}"
export PATH="$HOME/.cargo/bin:$PATH"

work="$(mktemp -d "${TMPDIR:-/tmp}/trovato-k3s.XXXXXX")"
trap 'rm -rf "$work"' EXIT

# ── Build ───────────────────────────────────────────────────────────────────
# scripts/build-overlay.sh copies the wasm from ./target, which is not where it
# lands when CARGO_TARGET_DIR points elsewhere, and writes overlay/ into the
# tree. So the same build runs here and the overlay is assembled in the staging
# directory instead.
echo "==> Building the site plugin for wasm32-wasip1"
cargo build --release --target wasm32-wasip1 -p trovato_site
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"

echo "==> Staging the image context"
ctx="$work/context"
mkdir -p "$ctx/plugins/trovato_site"
cp plugins/trovato_site/trovato_site.info.toml "$ctx/plugins/trovato_site/"
cp "$target_dir/wasm32-wasip1/release/trovato_site.wasm" "$ctx/plugins/trovato_site/"
if [ -d plugins/trovato_site/migrations ]; then
    cp -R plugins/trovato_site/migrations "$ctx/plugins/trovato_site/"
fi
# A snapshot, so the image is one consistent state of the tree even while
# somebody is editing it.
for d in templates static config; do cp -R "$d" "$ctx/$d"; done
# The image's copy of the plugin's base URL names the origin this deployment is
# served from, so its sitemap and llms.txt link within the review site.
set_plugin_base_url "$ctx/config" "http://${SITE_HOST}"
cp "$here/Dockerfile" "$ctx/Dockerfile"

sha="$(git rev-parse --short HEAD)"
if [ -n "$(git status --porcelain)" ]; then
    content_hash="$(cd "$ctx" && find . -type f -print0 | sort -z | xargs -0 shasum -a 256 | shasum -a 256 | cut -c1-8)"
    TAG="${sha}-dirty-${content_hash}"
else
    TAG="$sha"
fi
SITE_IMAGE="${IMAGE_REPO}:${TAG}"

echo "==> Building ${SITE_IMAGE} for linux/arm64 on ${KERNEL_IMAGE}"
docker build --platform linux/arm64 --build-arg "KERNEL_IMAGE=${KERNEL_IMAGE}" \
    -t "$SITE_IMAGE" "$ctx"
# registry.pozza takes anonymous pulls and authenticated pushes. If this Mac's
# docker has been logged in (docker login registry.pozza -u jeremyandrews), the
# push goes straight there. Otherwise the image is relayed through ks1, whose
# root podman holds the push login: docker save, piped over ssh into podman
# load, then podman push. The load goes into a throwaway image store made for
# this push and deleted after it (--root and --runroot), so ks1's own images
# are never read, retagged or removed. The password never leaves ks1.
echo "==> Pushing"
if docker push "$SITE_IMAGE" >"$work/push.log" 2>&1; then
    tail -n 1 "$work/push.log"
else
    if ! grep -q "no basic auth credentials\|unauthorized\|authorization" "$work/push.log"; then
        cat "$work/push.log" >&2; exit 1
    fi
    echo "    no push login here; relaying through ${PUSH_HOST:=ks1.pozza}"
    docker save "$SITE_IMAGE" | ssh -o BatchMode=yes "$PUSH_HOST" \
        "set -e; d=\$(sudo -n mktemp -d /var/tmp/trovato-site-push.XXXXXX); trap 'sudo -n rm -rf \$d' EXIT; sudo -n podman --root \$d/root --runroot \$d/run --storage-driver vfs load -q >/dev/null; sudo -n podman --root \$d/root --runroot \$d/run --storage-driver vfs push -q '$SITE_IMAGE'"
fi
# Confirm the registry has it, anonymously, the way the nodes will pull it.
curl -fsS -o /dev/null -H 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json' \
    "https://${IMAGE_REPO%%/*}/v2/${IMAGE_REPO#*/}/manifests/${TAG}" \
    || { echo "the registry does not have ${SITE_IMAGE} after the push" >&2; exit 1; }
echo "    registry has ${SITE_IMAGE}"

# ── Render ──────────────────────────────────────────────────────────────────
echo "==> Rendering manifests"
mkdir -p "$work/caddy"
cp "$here/Caddyfile" "$work/caddy/Caddyfile"
sed 's/site:3000/127.0.0.1:3000/g' deploy/shared.caddy > "$work/caddy/shared.caddy"
caddy_hash="$(cat "$work/caddy/Caddyfile" "$work/caddy/shared.caddy" | shasum -a 256 | cut -c1-12)"

rendered="$work/rendered.yaml"
{
    kubectl create configmap trovato-site-caddy -n "$NS" \
        --from-file="$work/caddy/Caddyfile" --from-file="$work/caddy/shared.caddy" \
        --dry-run=client -o yaml
    for f in postgres.yaml redis.yaml site.yaml; do
        echo "---"
        sed -e "s|__SITE_IMAGE__|${SITE_IMAGE}|g" \
            -e "s|__LB_IP__|${LB_IP}|g" \
            -e "s|__SITE_HOST__|${SITE_HOST}|g" \
            -e "s|__CADDY_HASH__|${caddy_hash}|g" "$here/$f"
    done
} > "$rendered"

# ── Diff, before anything is created ────────────────────────────────────────
# The server can only diff namespaced objects in a namespace that exists. On the
# first apply there is none, so the diff is every object, printed in full as the
# client renders it: all of it is new. Either way nothing has been created yet.
echo "==> kubectl diff"
if kubectl get namespace "$NS" >/dev/null 2>&1; then
    set +e
    kubectl diff -f "$here/namespace.yaml" -f "$rendered"
    rc=$?
    set -e
    case "$rc" in
        0) echo "    (no changes)" ;;
        1) ;;
        *) echo "kubectl diff failed" >&2; exit "$rc" ;;
    esac
else
    echo "    namespace ${NS} does not exist: everything below is new"
    kubectl apply --dry-run=client -o yaml -f "$here/namespace.yaml" -f "$rendered"
fi
if ! kubectl get namespace "$NS" >/dev/null 2>&1 || ! k get secret trovato-site-secrets >/dev/null 2>&1; then
    echo "    + secret/trovato-site-secrets (POSTGRES_PASSWORD, ADMIN_PASS, CRON_KEY; generated, never shown)"
fi

# ── Namespace and secret ────────────────────────────────────────────────────
if ! kubectl get namespace "$NS" >/dev/null 2>&1; then
    echo "==> Creating namespace ${NS}"
    kubectl apply -f "$here/namespace.yaml"
fi
if ! k get secret trovato-site-secrets >/dev/null 2>&1; then
    echo "==> Generating trovato-site-secrets (first apply; never written to disk)"
    k create secret generic trovato-site-secrets \
        --from-literal=POSTGRES_PASSWORD="$(openssl rand -hex 24)" \
        --from-literal=ADMIN_PASS="$(openssl rand -hex 16)" \
        --from-literal=CRON_KEY="$(openssl rand -hex 32)"
fi

echo "==> kubectl apply"
kubectl apply -f "$here/namespace.yaml" -f "$rendered"

echo "==> Waiting for the rollout"
k rollout status statefulset/postgres --timeout=300s
k rollout status statefulset/redis --timeout=300s
k rollout status deployment/trovato-site --timeout=600s

if [ "$POPULATE" = "1" ]; then
    SITE_SELECTOR="app=trovato-site"
    SITE_CONTAINER="site"
    restart_deployment() {
        k rollout restart deployment/trovato-site >/dev/null
        k rollout status deployment/trovato-site --timeout=600s
        wait_for_kernel
    }
    RESTART=restart_deployment
    ADMIN_PASS="$(secret_value trovato-site-secrets ADMIN_PASS)"
    CRON_KEY="$(secret_value trovato-site-secrets CRON_KEY)"
    populate
fi

# The installer is reached only from inside the pod. From the LAN it answers 404,
# so nobody can claim the administrator account in the moments between the
# Service coming up and populate posting the forms.
echo "==> Checking the installer is closed to the LAN"
install_code="$(curl -s -o /dev/null -w '%{http_code}' -m 10 "http://${LB_IP}/install/admin" || echo 000)"
if [ "$install_code" != "404" ]; then
    echo "/install/admin answered ${install_code} on the LAN address; expected 404" >&2
    exit 1
fi
echo "    /install/admin: 404"

echo
echo "The site is on http://${SITE_HOST}/ and http://${LB_IP}/ (image ${SITE_IMAGE})"
echo "Administrator: admin; password: kubectl -n ${NS} get secret trovato-site-secrets -o jsonpath='{.data.ADMIN_PASS}' | base64 -d"

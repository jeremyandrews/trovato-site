#!/usr/bin/env bash
# scripts/check-roundtrip.sh against the review site in k3s.
#
#   deploy/k3s/check-roundtrip.sh                 compare with the deployed config
#   ROUNDTRIP_AGAINST=worktree deploy/k3s/check-roundtrip.sh
#                                                 compare with config/ in this tree
#
# The script runs unmodified. Its two `docker compose` calls (exec the export,
# then cp the result out) go to shim/docker, which sends them to the site pod
# with kubectl instead. Read-only: export writes to /tmp in the container.
#
# What it compares with. The script reads `config/` beside itself. The image
# carries the snapshot of config/ it was built from, under /site/config, and
# that is what the site imported, so by default the comparison is with that:
# the script runs from a scratch root that links everything in this tree except
# config/, which is the image's copy. A tree edited since the last apply.sh then
# measures the deployment rather than the edits. ROUNDTRIP_AGAINST=worktree
# runs it in place, against the tree as it is now.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/ks1.yaml}"
export PATH="$here/shim:$HOME/.cargo/bin:$PATH"
export K3S_NS="${K3S_NS:-trovato-site}"
export K3S_SITE_SELECTOR="${K3S_SITE_SELECTOR:-app=trovato-site}"
export K3S_SITE_CONTAINER="${K3S_SITE_CONTAINER:-site}"
# Cargo builds the checks crate from wherever the script runs; keep its output
# where it already is rather than under the scratch root.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/target}"

if [ "${ROUNDTRIP_AGAINST:-image}" = "worktree" ]; then
    exec "$repo_root/scripts/check-roundtrip.sh" "$@"
fi

root="$(mktemp -d "${TMPDIR:-/tmp}/trovato-roundtrip.XXXXXX")"
trap 'rm -rf "$root"' EXIT
for entry in "$repo_root"/* "$repo_root"/.[!.]*; do
    [ -e "$entry" ] || continue
    case "$(basename "$entry")" in config|scripts|.git) continue ;; esac
    ln -s "$entry" "$root/"
done
# scripts/ is copied rather than linked: the script finds its root through its
# own directory, and a link would lead it back to this tree.
cp -R "$repo_root/scripts" "$root/scripts"
mkdir "$root/config"
docker compose cp "site:/site/config/." "$root/config/"
echo "==> Comparing with the config the deployed image carries"
"$root/scripts/check-roundtrip.sh" "$@"

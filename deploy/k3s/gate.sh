#!/usr/bin/env bash
# The repository's gates, against the review site in k3s.
#
#   deploy/k3s/gate.sh                     all six
#   deploy/k3s/gate.sh crawl roundtrip     just those
#
# Gates: crawl, front, a11y, shoot (the browser gates, npm scripts, pointed at the LAN
# address through SITE_BASE), roundtrip (check-roundtrip.sh, via kubectl) and
# moderation (check-moderation.sh, against a throwaway second site that is
# deleted afterwards). Every gate runs even when an earlier one fails; the exit
# status is the number of gates that failed.
#
# SITE_BASE defaults to the host SITE_URL names (http://10.20.250.17.nip.io),
# so the absolute URLs the site emits are on the origin the crawl walks.
# Screenshots go to a directory under the system temp dir, printed at the end.
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/../.." && pwd)"
LB_IP="${LB_IP:-10.20.250.17}"
export SITE_BASE="${SITE_BASE:-http://${LB_IP}.nip.io}"
export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/ks1.yaml}"

gates=("$@")
[ ${#gates[@]} -gt 0 ] || gates=(crawl front a11y shoot roundtrip moderation)

out="$(mktemp -d "${TMPDIR:-/tmp}/trovato-gates.XXXXXX")"
failed=0
summary=()

run_gate() {
    local name="$1"; shift
    echo
    echo "======== ${name} ========"
    local start=$SECONDS
    if "$@" 2>&1 | tee "$out/${name}.log"; then
        summary+=("PASS  ${name}  ($((SECONDS - start))s)")
    else
        summary+=("FAIL  ${name}  ($((SECONDS - start))s)  log: $out/${name}.log")
        failed=$((failed + 1))
    fi
}

# `npm run <x>` runs in the package root, and shoot writes ./screenshots there.
# The scripts are run with node directly from the temp directory instead, which
# is the same command minus the cwd, so nothing lands in the working tree.
browser_gate() {
    (cd "$out" && node "$repo_root/scripts/$1.mjs")
}

if [ ! -d "$repo_root/node_modules/playwright" ]; then
    (cd "$repo_root" && npm ci)
fi
(cd "$repo_root" && npx playwright install chromium >/dev/null)

echo "SITE_BASE=${SITE_BASE}"
for gate in "${gates[@]}"; do
    case "$gate" in
        crawl|front|a11y|shoot) run_gate "$gate" browser_gate "$gate" ;;
        roundtrip) run_gate roundtrip "$here/check-roundtrip.sh" ;;
        moderation) run_gate moderation "$here/check-moderation.sh" ;;
        *) echo "unknown gate: $gate" >&2; failed=$((failed + 1)) ;;
    esac
done

echo
echo "======== summary ========"
printf '%s\n' "${summary[@]}"
echo "logs and screenshots: ${out}"
exit "$failed"

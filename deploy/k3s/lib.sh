# Shared by apply.sh and the gate scripts. Sourced, not run.
#
# The population sequence is scripts/run.sh's, with `docker compose exec`
# replaced by `kubectl exec` and every HTTP call made from inside the site
# container against the kernel's own port, so none of it passes through Caddy
# or the LAN address and none of it spends the rate-limit budget the gates use.

export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/ks1.yaml}"

# The one namespace these scripts may touch. Not taken from the environment: an
# NS inherited from some other shell would otherwise send the secret, the
# throwaway moderation site and its label-selected cleanup somewhere else, while
# the manifests, which name trovato-site themselves, went here.
NS=trovato-site
require_namespace() {
    if [ "$1" != "trovato-site" ]; then
        echo "refusing namespace '$1': these scripts only touch trovato-site" >&2
        return 1
    fi
}
for _ns in "${NS}" "${K3S_NS:-trovato-site}"; do
    require_namespace "$_ns" || exit 1
done
unset _ns

# The plugins the site turns on. Same list, same order, as scripts/run.sh.
IMAGE_PLUGINS=(
    trovato_blog
    trovato_seo
    trovato_search
    trovato_contact
    trovato_scheduled_publishing
    trovato_redirects
    trovato_comments
    trovato_spam
)
SITE_PLUGINS=(trovato_site)

k() { kubectl -n "$NS" "$@"; }

# The running pod behind a label selector, e.g. `app=trovato-site`.
pod_for() {
    k get pods -l "$1" --field-selector=status.phase=Running \
        -o jsonpath='{.items[0].metadata.name}' 2>/dev/null
}

secret_value() {
    k get secret "$1" -o jsonpath="{.data.$2}" | base64 -d
}

# Everything below takes its target from these, which the caller sets:
#   SITE_SELECTOR   label selector of the site pod
#   SITE_CONTAINER  the kernel's container in it
#   RESTART         a function that restarts the kernel and returns once it is up
in_site() {
    local pod
    pod="$(pod_for "$SITE_SELECTOR")"
    [ -n "$pod" ] || { echo "no running pod for ${SITE_SELECTOR}" >&2; return 1; }
    k exec -i "$pod" -c "$SITE_CONTAINER" -- "$@"
}

# Wait until the kernel answers /health from inside its own container.
wait_for_kernel() {
    for _ in $(seq 1 90); do
        if in_site curl -fsS -o /dev/null -m 5 http://127.0.0.1:3000/health 2>/dev/null; then
            return 0
        fi
        sleep 2
    done
    echo "the kernel never became healthy" >&2
    return 1
}

# Name and status of every plugin, sorted: `plugin list` prints them in no
# stable order, and interleaves log lines.
plugin_states() {
    in_site ./trovato plugin list 2>/dev/null \
        | awk '$3 == "enabled" || $3 == "disabled" { print $1, $3 }' | sort || true
}

# What the installer's first page says about the site: `fresh` (200, nobody has
# claimed it), `installed` (a redirect away from it), or `error` for anything
# else, a 429 or a 500 or no answer at all, none of which says an administrator
# exists.
install_state() {
    case "$1" in
        200) echo fresh ;;
        301 | 302 | 303 | 307 | 308) echo installed ;;
        *) echo error ;;
    esac
}

# Fails, naming each one, unless every plugin in the arguments is `enabled` in
# the `name state` lines on stdin.
require_enabled() {
    local states missing="" name
    states="$(cat)"
    for name in "$@"; do
        if ! printf '%s\n' "$states" | grep -qx "${name} enabled"; then
            missing="${missing} ${name}"
        fi
    done
    if [ -n "$missing" ]; then
        echo "not enabled:${missing}" >&2
        return 1
    fi
}

# Points the site plugin's base URL at the origin it is actually served from.
# The plugin builds the sitemap and llms.txt from this variable, not from
# SITE_URL, which it cannot read; left at the production value, every link in
# them would send a reader of the review site to trovato.rs.
set_plugin_base_url() {
    local config_dir="$1" base="$2"
    local file="$config_dir/variable.plugin.trovato_site.site_base_url.yml"
    [ -f "$file" ] || { echo "missing $file" >&2; return 1; }
    sed "s|^value: .*\$|value: ${base}|" "$file" > "$file.new"
    mv "$file.new" "$file"
    grep -qx "value: ${base}" "$file"
}

# The run.sh install sequence. Needs ADMIN_PASS and CRON_KEY set.
populate() {
    local admin_user="${ADMIN_USER:-admin}"
    local admin_mail="${ADMIN_MAIL:-admin@trovato.rs}"
    local site_name="${SITE_NAME:-Trovato}"
    local site_slogan="${SITE_SLOGAN:-A content management system written in Rust.}"

    wait_for_kernel

    echo "==> Installing plugins"
    local before after name
    before="$(plugin_states)"
    for name in "${IMAGE_PLUGINS[@]}" "${SITE_PLUGINS[@]}"; do
        in_site ./trovato plugin install "$name" >/dev/null 2>&1 \
            || in_site ./trovato plugin enable "$name" >/dev/null 2>&1 \
            || true
        echo "    ${name}"
    done
    # Migrations a newer build of an already installed plugin brings with it.
    in_site ./trovato plugin migrate >/dev/null \
        || { echo "plugin migrate failed" >&2; return 1; }
    after="$(plugin_states)"
    printf '%s\n' "$after" | require_enabled "${IMAGE_PLUGINS[@]}" "${SITE_PLUGINS[@]}" || return 1

    # A plugin's content types register when the kernel loads it, so a change
    # to the set of enabled plugins needs a restart before config import.
    if [ "$before" != "$after" ]; then
        echo "==> Restarting the site so the new plugins are loaded"
        "$RESTART"
    else
        echo "    (no plugin changed state; no restart needed)"
    fi

    echo "==> Running the installer"
    # /install/admin answers 200 while the site is not installed and redirects
    # once it is. Only a fresh site gets the forms posted at it.
    local code
    code="$(in_site curl -sS -o /dev/null -w '%{http_code}' http://127.0.0.1:3000/install/admin || echo 000)"
    case "$(install_state "$code")" in
        fresh) ;;
        installed)
            echo "    (already installed: /install/admin answered ${code})" ;;
        *)
            echo "the installer answered ${code}; cannot tell whether the site is installed" >&2
            return 1 ;;
    esac
    if [ "$(install_state "$code")" = fresh ]; then
        in_site sh -c '
            set -e
            jar=$(mktemp)
            trap "rm -f $jar" EXIT
            curl -fsS -c "$jar" -o /dev/null -X POST http://127.0.0.1:3000/install/admin \
                --data-urlencode "username=$1" --data-urlencode "email=$2" \
                --data-urlencode "password=$3" --data-urlencode "password_confirm=$3"
            curl -fsS -b "$jar" -o /dev/null -X POST http://127.0.0.1:3000/install/site \
                --data-urlencode "site_name=$4" --data-urlencode "site_slogan=$5" \
                --data-urlencode "site_mail=$2"
        ' sh "$admin_user" "$admin_mail" "$ADMIN_PASS" "$site_name" "$site_slogan"
        echo "    installed (administrator: ${admin_user})"
    fi

    echo "==> Importing the site config"
    in_site ./trovato config import /site/config | tail -n 3
    echo "==> Importing the mirrored documentation"
    in_site ./trovato config import /site/config/docs | tail -n 3

    # Gather queries and menu links are read into their registries at start.
    echo "==> Restarting the site so the imported config is live"
    "$RESTART"

    echo "==> Running cron once"
    in_site sh -c 'curl -fsS -o /dev/null -m 60 -X POST "http://127.0.0.1:3000/cron/$1"' sh "$CRON_KEY" \
        || { echo "the cron tick failed" >&2; return 1; }

    # The cron tick is what copies the Italian overlays into place, so a site
    # that came up without them is not a finished deployment.
    echo "==> Checking the Italian pages"
    local page
    page="$(in_site curl -fsS -m 30 http://127.0.0.1:3000/it/why)" \
        || { echo "/it/why did not load" >&2; return 1; }
    case "$page" in
        *'lang="it"'*) ;;
        *) echo "/it/why is not served in Italian" >&2; return 1 ;;
    esac
    echo "    /it/why is Italian"
}

#!/usr/bin/env bash
#
# Write the pinned Trovato release from kernel-release.toml into every file that
# names it. kernel-release.toml is the only place the release is authored; this
# script is how that one fact reaches the ten-odd places that repeat it, and
# checks/tests/kernel_release.rs is what fails when they disagree.
#
#   scripts/sync-kernel-release.sh                  rewrite from the current pin
#   scripts/sync-kernel-release.sh --set-version X.Y.Z
#                                                   resolve the tag, then rewrite
#
# Run it from anywhere: the repository root is resolved from this file's location.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
contract="$root/kernel-release.toml"

die() {
    echo "sync-kernel-release: $*" >&2
    exit 1
}

# Read a bare `key = "value"` out of the contract file.
field() {
    local key="$1"
    sed -n "s/^${key} = \"\\(.*\\)\"\$/\\1/p" "$contract" | head -n 1
}

# Replace the whole file with the result of a sed program, leaving the file
# untouched if the program changes nothing. Written through a temporary file
# because `sed -i` spells its argument differently on BSD and on GNU.
rewrite() {
    local path="$1"
    shift
    local tmp
    tmp="$(mktemp)"
    sed "$@" "$path" > "$tmp"
    if cmp -s "$path" "$tmp"; then
        rm -f "$tmp"
    else
        cat "$tmp" > "$path"
        rm -f "$tmp"
        echo "  rewrote ${path#"$root"/}"
    fi
}

# Replace the lines between `<!-- kernel-release:begin -->` and its matching end
# marker with the contents of the given file, keeping the markers themselves.
# The body arrives as a file because BSD awk will not take a newline in `-v`.
write_block() {
    local path="$1" body_file="$2"
    grep -q '<!-- kernel-release:begin -->' "$path" ||
        die "$path has no kernel-release:begin marker"
    local tmp
    tmp="$(mktemp)"
    awk -v body_file="$body_file" '
        /<!-- kernel-release:begin -->/ {
            print
            while ((getline line < body_file) > 0) print line
            close(body_file)
            skip = 1
            next
        }
        /<!-- kernel-release:end -->/   { skip = 0 }
        !skip                          { print }
    ' "$path" > "$tmp"
    if cmp -s "$path" "$tmp"; then
        rm -f "$tmp"
    else
        cat "$tmp" > "$path"
        rm -f "$tmp"
        echo "  rewrote ${path#"$root"/}"
    fi
}

[ -f "$contract" ] || die "$contract is missing"

if [ "${1:-}" = "--set-version" ]; then
    new="${2:-}"
    echo "$new" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' ||
        die "--set-version wants a version like 0.102.0, got '${new}'"
    echo "resolving v${new} ..."
    # `^{}` asks for the commit a tag points at, so an annotated tag resolves to
    # the commit rather than to the tag object.
    resolved="$(git ls-remote https://github.com/jeremyandrews/trovato.git \
        "refs/tags/v${new}^{}" | cut -f 1)"
    [ -n "$resolved" ] ||
        die "v${new} does not resolve to a commit in jeremyandrews/trovato"
    rewrite "$contract" \
        -e "s/^version = \".*\"\$/version = \"${new}\"/" \
        -e "s/^rev = \".*\"\$/rev = \"${resolved}\"/"
elif [ -n "${1:-}" ]; then
    die "unknown argument '$1'"
fi

version="$(field version)"
rev="$(field rev)"
[ -n "$version" ] || die "kernel-release.toml has no version"
[ -n "$rev" ] || die "kernel-release.toml has no rev"

api="${version%.*}"
short="$(printf '%.8s' "$rev")"

echo "Trovato ${version} (tag v${version}, commit ${short}), plugin API ${api}"

# The dependency rev. Cargo cannot read this out of another file, so the literal
# stays in Cargo.toml and the test is what keeps it honest.
rewrite "$root/Cargo.toml" \
    -e "/^trovato-sdk = /s/rev = \"[0-9a-f]*\"/rev = \"${rev}\"/"

# The plugin's declared API version, which the kernel refuses at load if it is
# higher than the kernel's own.
rewrite "$root/plugins/trovato_site/trovato_site.info.toml" \
    -e "s/^api_version = \".*\"\$/api_version = \"${api}\"/"

# The kernel image tag, as the environment variable and as the fallbacks that
# make the documented commands work without one.
rewrite "$root/.env.example" -e "s/^TROVATO_VERSION=.*\$/TROVATO_VERSION=${version}/"
rewrite "$root/.env.production.example" -e "s/^TROVATO_VERSION=.*\$/TROVATO_VERSION=${version}/"
for f in docker-compose.yml docker-compose.production.yml \
    docker-compose.production-local.yml scripts/check-production.sh; do
    [ -f "$root/$f" ] || continue
    rewrite "$root/$f" -e "s/\${TROVATO_VERSION:-[^}]*}/\${TROVATO_VERSION:-${version}}/g"
done

# The tag the documentation importer mirrors from.
rewrite "$root/tools/src/docs_import.rs" \
    -e "s/^const TAG: &str = \".*\";\$/const TAG: \&str = \"v${version}\";/"

readme_block="$(mktemp)"
cat > "$readme_block" <<BLOCK
The site is built against Trovato \`${version}\` (tag \`v${version}\`, commit
\`${short}\`): the SDK is pinned to that commit, the plugin declares
\`api_version = "${api}"\`, and the stack runs
\`ghcr.io/jeremyandrews/trovato:${version}\`.
BLOCK
write_block "$root/README.md" "$readme_block"
rm -f "$readme_block"

provenance_block="$(mktemp)"
cat > "$provenance_block" <<BLOCK
The site currently runs Trovato \`${version}\` (tag \`v${version}\`).
BLOCK
write_block "$root/static/brand/PROVENANCE.md" "$provenance_block"
rm -f "$provenance_block"

echo "done"

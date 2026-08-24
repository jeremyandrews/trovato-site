#!/usr/bin/env bash
# Build the site plugin and assemble the overlay the kernel mounts.
#
# The kernel discovers a plugin as a directory under a PLUGINS_DIR entry holding
# `<name>.info.toml` next to `<name>.wasm`. Cargo puts the .wasm under target/,
# so the two halves are brought together here rather than by mounting a source
# tree that does not have the shape the kernel expects.
#
# Idempotent: run it as often as you like.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

plugins=(trovato_site)
target=wasm32-wasip1

echo "==> Building site plugins for ${target}"
cargo build --release --target "${target}"

echo "==> Assembling overlay/"
rm -rf overlay/plugins
for name in "${plugins[@]}"; do
    dest="overlay/plugins/${name}"
    mkdir -p "${dest}"
    cp "plugins/${name}/${name}.info.toml" "${dest}/"
    cp "target/${target}/release/${name}.wasm" "${dest}/"
    if [ -d "plugins/${name}/migrations" ]; then
        cp -R "plugins/${name}/migrations" "${dest}/"
    fi
    echo "    ${dest}: $(ls "${dest}" | tr '\n' ' ')"
done

echo "==> Overlay ready"

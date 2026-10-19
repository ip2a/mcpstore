#!/usr/bin/env bash
# Sync icon assets for the wasm/web build: the web Assets source fetches
# icons/*.svg over HTTP, so vite serves them from www/public/icons.
# Icons come from the gpui-kit-assets crate already in the local cargo registry.
set -euo pipefail

crate_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
registry_dir="$(find ~/.cargo/registry/src -maxdepth 2 -type d -name 'gpui-kit-assets-*' 2>/dev/null | sort | tail -1)"
if [ -z "$registry_dir" ]; then
  echo "gpui-kit-assets not found in cargo registry (run any cargo build once)" >&2
  exit 1
fi

mkdir -p "$crate_dir/www/public/assets"
rm -rf "$crate_dir/www/public/assets/icons"
cp -R "$registry_dir/assets/icons" "$crate_dir/www/public/assets/icons"
echo "[web-assets] $(find "$crate_dir/www/public/assets/icons" -name '*.svg' | wc -l | tr -d ' ') icons from $registry_dir"

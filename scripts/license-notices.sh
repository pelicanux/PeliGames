#!/usr/bin/env bash
# Generate the dependency notices shipped in the packages (licenses/generated/).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
metadata=$(mktemp -d /tmp/peligames-license-metadata-XXXXXX)
trap 'rm -rf "$metadata"' EXIT
# The launcher, PeliRun and the in-game overlay have separate locked Cargo graphs.
manifests=(src-tauri/Cargo.toml src-tauri/launch-runtime/Cargo.toml tools/game-overlay/Cargo.toml)
files=()
for manifest in "${manifests[@]}"; do
  file="$metadata/${manifest//\//_}.json"
  cargo metadata --format-version 1 --locked --offline --filter-platform x86_64-unknown-linux-gnu \
    --manifest-path "$root/$manifest" > "$file"
  files+=("$file")
done
bun "$root/scripts/generate-license-notices.ts" "${files[@]}"

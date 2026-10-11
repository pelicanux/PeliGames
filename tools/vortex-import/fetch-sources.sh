#!/usr/bin/env bash
# Baixa as revisões fixadas em sources.txt para um cache persistente.
# Uso: tools/vortex-import/fetch-sources.sh [DIR_DO_CACHE]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
CACHE="${1:-${XDG_CACHE_HOME:-$HOME/.cache}/peligames/vortex-sources}"
mkdir -p "$CACHE"
while read -r repo rev; do
  [[ -z "${repo:-}" || "$repo" == \#* ]] && continue
  dest="$CACHE/$repo"
  if [[ -f "$dest/.revision" && "$(cat "$dest/.revision")" == "$rev" ]]; then
    continue
  fi
  echo "baixando $repo@$rev" >&2
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  curl -fsSL --retry 3 "https://codeload.github.com/$repo/tar.gz/$rev" -o "$tmp/src.tar.gz"
  mkdir -p "$tmp/out"
  if [[ "$repo" == "Nexus-Mods/Vortex" ]]; then
    # Somente as extensões de jogos; o restante do Vortex não é lido.
    tar -xzf "$tmp/src.tar.gz" -C "$tmp/out" --strip-components=1 --wildcards '*/extensions/games/*'
  else
    tar -xzf "$tmp/src.tar.gz" -C "$tmp/out" --strip-components=1
  fi
  rm -rf "$dest"
  mkdir -p "$(dirname "$dest")"
  mv "$tmp/out" "$dest"
  echo "$rev" > "$dest/.revision"
  rm -rf "$tmp"
  trap - EXIT
done < "$HERE/sources.txt"
echo "$CACHE"

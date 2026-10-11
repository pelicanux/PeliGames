#!/usr/bin/env bash
# Extrai os fatos das extensões Vortex e gera o relatório comparativo.
# Uso: tools/vortex-import/run.sh [--emit] [DIR_DAS_FONTES]
# Sem diretório, usa (e baixa, se preciso) o cache das revisões de sources.txt.
# --emit regrava src-tauri/resources/nexus-games/<família>-vortex*.json.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
EMIT=()
if [[ "${1:-}" == "--emit" ]]; then shift; EMIT=(--emit src-tauri/resources/nexus-games); fi
SRC="${1:-$(tools/vortex-import/fetch-sources.sh)}"
if (( ${#EMIT[@]} )); then
  # Listas públicas do Nexus (sem chave de API) para validar domínio e extensão publicada.
  INDEX="${XDG_CACHE_HOME:-$HOME/.cache}/peligames/nexus-index"
  mkdir -p "$INDEX"
  curl -fsSL --retry 3 https://data.nexusmods.com/file/nexus-data/games.json -o "$INDEX/games.json"
  curl -fsSL --retry 3 https://api.nexusmods.com/v3/vortex/extensions -o "$INDEX/extensions.json"
  EMIT+=(--nexus-index "$INDEX/games.json" "$INDEX/extensions.json")
fi
cargo build --release --quiet --manifest-path tools/vortex-import/Cargo.toml
rev() { awk -v r="$1" '$1 == r { print $2 }' tools/vortex-import/sources.txt; }
args=(--source "vortex=$SRC/Nexus-Mods/Vortex/extensions/games"
      --source "chemboy1=$SRC/ChemGuy1611/ChemBoy1-Vortex-Games"
      --evidence "vortex=https://github.com/Nexus-Mods/Vortex/blob/$(rev Nexus-Mods/Vortex)/extensions/games"
      --evidence "chemboy1=https://github.com/ChemGuy1611/ChemBoy1-Vortex-Games/blob/$(rev ChemGuy1611/ChemBoy1-Vortex-Games)")
for repo in "$SRC"/Nexus-Mods/game-*; do
  [ -d "$repo" ] && args+=(--source "$(basename "$repo")=$repo")
done
rm -rf tools/vortex-import/generated
tools/vortex-import/target/release/peligames-vortex-import "${args[@]}" "${EMIT[@]}" \
  --modules src-tauri/resources/nexus-games --out tools/vortex-import/generated

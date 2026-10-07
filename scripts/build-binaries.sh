#!/usr/bin/env bash
set -euo pipefail
project_root=$(cd "$(dirname "$0")/.." && pwd)
cd "$project_root"
build_target=$(mktemp -d /tmp/peligames-binaries-XXXXXX)
trap 'rm -rf "$build_target"' EXIT
export CARGO_TARGET_DIR="$build_target"
export CARGO_BUILD_JOBS=$(nproc)
export CARGO_INCREMENTAL=0
export PATH="${BUN_INSTALL:-$HOME/.bun}/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
findmnt -no TARGET,FSTYPE -T "$build_target"
bun run build
cargo build --manifest-path src-tauri/Cargo.toml --release --bins --features custom-protocol --offline -j "$CARGO_BUILD_JOBS"
"$build_target/release/dlssnr-x-amd" --verify-ui
"$build_target/release/Pelinstall" --verify-ui
mkdir -p Release
install -m755 "$build_target/release/dlssnr-x-amd" Release/.PeliGames.new
install -m755 "$build_target/release/Pelinstall" Release/.Pelinstall.new
mv -f Release/.PeliGames.new Release/PeliGames
mv -f Release/.Pelinstall.new Release/Pelinstall
python3 - <<'PY'
from pathlib import Path
root = Path.cwd()
def arg(value):
    return '"' + value.replace('\\', '\\\\\\\\').replace('"', '\\\\"').replace('`', '\\\\`').replace('$', '\\\\$').replace('%', '%%') + '"'
entry = root / 'Release/Pelinstall.desktop'
entry.write_text('[Desktop Entry]\nVersion=1.0\nType=Application\nName=Pelinstall\nComment=Instalar jogos e programas Windows com PeliGames\nExec=' + arg(str(root / 'Release/Pelinstall')) + ' %f\nIcon=' + str(root / 'public/peligames.png') + '\nTerminal=false\nCategories=Game;\nMimeType=application/x-ms-dos-executable;application/x-msdownload;application/x-msi;\n')
entry.chmod(0o755)
PY
file Release/PeliGames Release/Pelinstall
sha256sum Release/PeliGames Release/Pelinstall

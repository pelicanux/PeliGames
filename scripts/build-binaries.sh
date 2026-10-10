#!/usr/bin/env bash
set -euo pipefail
# Use the timezone passed by the host; otherwise retain the system timezone.
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
"$build_target/release/peligames" --verify-ui
"$build_target/release/pelinstall" --verify-ui
mkdir -p Release
install -m755 "$build_target/release/peligames" Release/.peligames.new
install -m755 "$build_target/release/pelinstall" Release/.pelinstall.new
mv -f Release/.peligames.new Release/peligames
mv -f Release/.pelinstall.new Release/pelinstall
python3 - <<'PY'
from pathlib import Path
root = Path.cwd()
def arg(value):
    return '"' + value.replace('\\', '\\\\\\\\').replace('"', '\\\\"').replace('`', '\\\\`').replace('$', '\\\\$').replace('%', '%%') + '"'
entry = root / 'Release/pelinstall.desktop'
entry.write_text('[Desktop Entry]\nVersion=1.0\nType=Application\nName=Pelinstall\nComment=Instalando jogos na base da humilhação\nExec=' + arg(str(root / 'Release/pelinstall')) + ' %f\nIcon=' + str(root / 'public/peligames.svg') + '\nTerminal=false\nCategories=Game;\nMimeType=application/x-ms-dos-executable;application/x-msdownload;application/x-msi;\n')
entry.chmod(0o755)
PY
file Release/peligames Release/pelinstall
sha256sum Release/peligames Release/pelinstall

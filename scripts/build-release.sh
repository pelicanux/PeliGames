#!/usr/bin/env bash
set -euo pipefail
# Use the timezone passed by the host; otherwise retain the system timezone.
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
bundles=${1:-deb,rpm,appimage}
build_target=$(mktemp -d /tmp/peligames-release-XXXXXX)
trap 'rm -rf "$build_target"' EXIT
if [[ $(findmnt -no FSTYPE -T "$build_target" | sort -u) != tmpfs ]]; then
    echo "A compilação precisa de /tmp em memória RAM (tmpfs)." >&2
    exit 1
fi
export PELIGAMES_FRONTEND_DIST="$build_target/ui"
# The mktemp path only has letters, digits, '/', '-' and '_': safe in a JSON string.
export TAURI_CONFIG="{\"build\":{\"frontendDist\":\"$PELIGAMES_FRONTEND_DIST\"}}"
export CARGO_TARGET_DIR="$build_target"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-$(nproc)}
export CARGO_INCREMENTAL=0
export PATH="${BUN_INSTALL:-$HOME/.bun}/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
source scripts/native-voice-toolchain.sh
bun run build
cargo build --manifest-path src-tauri/Cargo.toml --release --bins --features custom-protocol --offline -j "$CARGO_BUILD_JOBS"
"$build_target/release/peligames" --verify-ui
"$build_target/release/pelinstall" --verify-ui
mkdir -p Release/licenses
cp -a licenses/. Release/licenses/
install -m644 LICENSE Release/licenses/PeliGames-GPL-3.0.txt
install -m755 "$build_target/release/peligames" Release/.peligames.new
install -m755 "$build_target/release/pelinstall" Release/.pelinstall.new
mv -f Release/.peligames.new Release/peligames
mv -f Release/.pelinstall.new Release/pelinstall
if [[ $EUID -eq 0 ]]; then
    artifact_owner=$(stat -c '%u:%g' .)
    chown "$artifact_owner" Release Release/peligames Release/pelinstall
    chown -R "$artifact_owner" Release/licenses
    if [[ -f Release/pelinstall.desktop ]]; then chown "$artifact_owner" Release/pelinstall.desktop; fi
fi
# linuxdeploy can run without a FUSE mount inside a container.
export APPIMAGE_EXTRACT_AND_RUN=1
bash scripts/tauri.sh bundle --features custom-protocol --bundles "$bundles"

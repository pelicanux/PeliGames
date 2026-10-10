#!/usr/bin/env bash
set -euo pipefail
# Use the timezone passed by the host; otherwise retain the system timezone.
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
build_target=$(mktemp -d /tmp/peligames-release-XXXXXX)
trap 'rm -rf "$build_target"' EXIT
export CARGO_TARGET_DIR="$build_target"
export CARGO_BUILD_JOBS=$(nproc)
export CARGO_INCREMENTAL=0
export PATH="${BUN_INSTALL:-$HOME/.bun}/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
bun run build
cargo build --manifest-path src-tauri/Cargo.toml --release --bins --features custom-protocol --offline -j "$CARGO_BUILD_JOBS"
"$build_target/release/peligames" --verify-ui
"$build_target/release/pelinstall" --verify-ui
mkdir -p Release
install -m755 "$build_target/release/peligames" Release/.peligames.new
install -m755 "$build_target/release/pelinstall" Release/.pelinstall.new
mv -f Release/.peligames.new Release/peligames
mv -f Release/.pelinstall.new Release/pelinstall
# linuxdeploy can run without a FUSE mount inside a container.
export APPIMAGE_EXTRACT_AND_RUN=1
bun run tauri bundle --features custom-protocol --bundles deb,rpm,appimage

#!/usr/bin/env bash
# No Wine session, display, game, C compiler or user profile is used.
set -euo pipefail
root=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d /tmp/peligames-wine-input-probe-XXXXXX)
trap 'rm -rf "$work"' EXIT
CARGO_INCREMENTAL=0 cargo build --release --locked --offline --manifest-path "$root/../../audit/Cargo.toml" --target-dir "$work/audit"
for abi in cursor_one cursor_two; do
  directory="$work/$abi"
  mkdir "$directory"
  rustc --edition=2021 --crate-type cdylib "$root/renderer.rs" -o "$directory/libpeligames_game_overlay.so"
  rustc --edition=2021 --crate-type cdylib "$root/ntdll.rs" -o "$directory/ntdll.so"
  rustc --edition=2021 --cfg "$abi" --crate-type cdylib "$root/win32u.rs" -o "$directory/win32u.so"
  rustc --edition=2021 --cfg "$abi" "$root/probe.rs" -o "$directory/probe"
  printf 'Testing Wine cursor ABI: %s\n' "$abi"
  LD_PRELOAD="$directory/libpeligames_game_overlay.so" LD_AUDIT="$work/audit/release/libpeligames_overlay_audit.so" "$directory/probe" "$directory"
done

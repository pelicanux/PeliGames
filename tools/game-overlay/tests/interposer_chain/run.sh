#!/usr/bin/env bash
# Synthetic ELF DSOs only: no display, Wine session, game or user profile.
set -euo pipefail
root=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d /tmp/peligames-interposer-probe-XXXXXX)
trap 'rm -rf "$work"' EXIT
CARGO_INCREMENTAL=0 cargo build --release --locked --offline --manifest-path "$root/../../audit/Cargo.toml" --target-dir "$work/audit"
for source in renderer x11 interposer dlsym_shim; do
  rustc --edition=2021 --crate-type cdylib "$root/$source.rs" -o "$work/$source.so"
done
mv "$work/x11.so" "$work/libX11.so.6"
# GE reads &XCheckIfEvent directly from its GOT as well as calling dlsym
# through the PLT. Audit can change the latter, but not the GOT relocation.
cc -shared -fPIC -fplt -Wall -Wextra -Werror "$root/bridge.c" -L"$work" -Wl,-rpath,'$ORIGIN' -l:libX11.so.6 -ldl -o "$work/bridge.so"
mv "$work/renderer.so" "$work/libpeligames_game_overlay.so"
mv "$work/bridge.so" "$work/liblsteamclient_overlay_bridge.so.1"
cp "$work/interposer.so" "$work/gameoverlayrenderer.so"
cp "$work/liblsteamclient_overlay_bridge.so.1" "$work/libVkLayer_GE_Wayland_SteamOverlay.so"
cp "$work/interposer.so" "$work/interposer0.so"
cp "$work/interposer.so" "$work/interposer1.so"
rustc --edition=2021 "$root/probe.rs" -o "$work/probe"
rustc --edition=2021 "$root/preload.rs" -o "$work/preload"
preload="$work/libpeligames_game_overlay.so"
for count in 0 1 2; do
  PROBE_X11="$work/libX11.so.6" LD_PRELOAD="$preload" LD_AUDIT="$work/audit/release/libpeligames_overlay_audit.so" "$work/probe" "$work" "$count"
  preload="$preload:$work/interposer$count.so"
done

PROBE_DLSYM_SHIM=1 PROBE_X11="$work/libX11.so.6" LD_PRELOAD="$work/libpeligames_game_overlay.so:$work/dlsym_shim.so" LD_AUDIT="$work/audit/release/libpeligames_overlay_audit.so" "$work/probe" "$work" 0

# Build every Wayland order through the production runner helper. GE must
# discover Steam through BOTH its direct binding and RTLD_DEFAULT lookup.
cp "$work/dlsym_shim.so" "$work/libMangoHud_shim.so"
for existing in \
  "$work/gameoverlayrenderer.so" \
  "$work/libMangoHud_shim.so:$work/gameoverlayrenderer.so" \
  "$work/gameoverlayrenderer.so:/legacy/libpeligames_game_overlay.so:$work/libMangoHud_shim.so"; do
  preload=$("$work/preload" "$work/libpeligames_game_overlay.so" "$existing" 1)
  PROBE_STEAM_DISCOVERY_ONLY=1 PROBE_X11="$work/libX11.so.6" LD_PRELOAD="$preload" LD_AUDIT="$work/audit/release/libpeligames_overlay_audit.so" "$work/probe" "$work" 0
done

# Selecting X11 must keep the existing PeliGames -> Steam -> X11 event chain.
preload=$("$work/preload" "$work/libpeligames_game_overlay.so" "$work/gameoverlayrenderer.so" 0)
PROBE_X11_PRIORITY=1 PROBE_X11="$work/libX11.so.6" LD_PRELOAD="$preload" LD_AUDIT="$work/audit/release/libpeligames_overlay_audit.so" "$work/probe" "$work" 0

#!/usr/bin/env bash
# Verify the library in the final image, after linuxdeploy has copied/stripped it.
set -euo pipefail
image=${1:?Usage: verify.sh AppImage expected-libgdk-3.so.0}
expected=${2:?Usage: verify.sh AppImage expected-libgdk-3.so.0}
image=$(realpath "$image")
expected=$(realpath "$expected")
work=$(mktemp -d /tmp/peligames-gdk-verify-XXXXXX)
trap 'rm -rf "$work"' EXIT
(cd "$work" && APPIMAGE_EXTRACT_AND_RUN=1 "$image" --appimage-extract usr/lib/libgdk-3.so.0 >/dev/null)
actual="$work/squashfs-root/usr/lib/libgdk-3.so.0"
[[ -f "$actual" ]] || { echo 'GDK ausente do AppImage final.' >&2; exit 1; }
build_id() { LC_ALL=C readelf -n "$1" | awk '/Build ID:/ {print $3; exit}'; }
expected_id=$(build_id "$expected")
actual_id=$(build_id "$actual")
[[ -n "$expected_id" && "$actual_id" == "$expected_id" ]] || {
  echo 'O empacotador substituiu a biblioteca GDK corrigida. AppImage recusado.' >&2; exit 1;
}
printf 'GDK do AppImage conferido (build-id %s).\n' "$actual_id"

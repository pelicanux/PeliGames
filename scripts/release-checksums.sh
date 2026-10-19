#!/usr/bin/env bash
# Lists the SHA-256 of the executables inside each package of Release/, as
# PeliGames_<version>_binaries.sha256. Published with the release, it lets the
# launcher tell an official build from a local one (commands/official_build.rs).
# Runs on the host: needs bsdtar (reads .deb and .rpm) and sha256sum.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
version=$(bun -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).version)' -- "$root/src-tauri/tauri.conf.json")
release="$root/Release"
appimage="$release/PeliGames-x86_64.AppImage"
deb="$release/PeliGames_${version}_amd64.deb"
rpm="$release/PeliGames-${version}-1.x86_64.rpm"
for package in "$appimage" "$deb" "$rpm"; do
  [[ -f "$package" ]] || { echo "Pacote ausente: $package" >&2; exit 1; }
done
# The AppImage name has no version: it must come from a build after the version bump.
[[ "$appimage" -nt "$root/src-tauri/tauri.conf.json" ]] || { echo "PeliGames-x86_64.AppImage é anterior à versão $version. Compile de novo." >&2; exit 1; }
command -v bsdtar >/dev/null || { echo 'Instale bsdtar (libarchive) para ler os pacotes.' >&2; exit 1; }
work=$(mktemp -d /tmp/peligames-checksums-XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/appimage" "$work/deb" "$work/rpm"
# The AppImage runtime extracts without FUSE.
(cd "$work/appimage" && APPIMAGE_EXTRACT_AND_RUN=1 "$appimage" --appimage-extract >/dev/null)
bsdtar -xOf "$deb" 'data.tar*' | bsdtar -xf - -C "$work/deb"
bsdtar -xf "$rpm" -C "$work/rpm"
output="$release/PeliGames_${version}_binaries.sha256"
{
  for format in appimage deb rpm; do
    while IFS= read -r -d '' file; do
      printf '%s  %s/%s\n' "$(sha256sum "$file" | cut -d' ' -f1)" "$format" "${file#"$work/$format/"}"
    done < <(find "$work/$format" -type f \( -name peligames -o -name pelinstall \) -print0 | sort -z)
  done
} > "$output.part"
[[ $(wc -l < "$output.part") -ge 6 ]] || { rm -f "$output.part"; echo 'Executáveis não encontrados em todos os pacotes.' >&2; exit 1; }
mv -f "$output.part" "$output"
echo "Hashes dos executáveis: $output"

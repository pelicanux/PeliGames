#!/usr/bin/env bash
set -euo pipefail
# Publish installable packages from Release; standalone binaries stay local.
repo=pelicanux/PeliGames
root=$(cd "$(dirname "$0")/.." && pwd)
version=$(bun -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).version)' -- "$root/src-tauri/tauri.conf.json")
tag="v$version"
bun -e '
const [root, expected] = process.argv.slice(1);
const { readFileSync } = require("fs");
const pkg = JSON.parse(readFileSync(root + "/package.json", "utf8")).version;
const cargo = /^version\s*=\s*"([^"\n]+)"/m.exec(readFileSync(root + "/src-tauri/Cargo.toml", "utf8"))?.[1];
if (pkg !== expected || cargo !== expected) { console.error("Versions differ. Update package.json, Cargo.toml and tauri.conf.json together."); process.exit(1); }
' -- "$root" "$version"
notes=${1:?Usage: scripts/publish-release.sh /path/to/release-notes.md}
[[ -f "$notes" ]] || { echo 'Release notes file not found.' >&2; exit 1; }
command -v gh >/dev/null || { echo 'Install GitHub CLI and run gh auth login.' >&2; exit 1; }
# Only select packages with the current version; never upload old builds.
mapfile -d '' packages < <(find "$root/Release" -maxdepth 1 -type f \( -name "PeliGames-x86_64.AppImage" -o -name "*_${version}_*.deb" -o -name "*-${version}-*.rpm" \) -print0)
# The AppImage name has no version: it must come from a build after the version bump.
[[ "$root/Release/PeliGames-x86_64.AppImage" -nt "$root/src-tauri/tauri.conf.json" ]] || { echo "PeliGames-x86_64.AppImage é anterior à versão $version. Compile de novo." >&2; exit 1; }
for format in AppImage deb rpm; do
  found=false
  for package in "${packages[@]}"; do [[ "$package" == *."$format" ]] && found=true; done
  [[ "$found" == true ]] || { echo "Missing $format for $version. Run bun run tauri build first." >&2; exit 1; }
done
# Executable hashes: the launcher checks itself against them (official build).
sums="$root/Release/PeliGames_${version}_binaries.sha256"
[[ -f "$sums" ]] || { echo "Missing $(basename "$sums"). Run scripts/release-checksums.sh first." >&2; exit 1; }
packages+=("$sums")
# Modified GDK is dynamically linked inside the AppImage; distribute its source too.
gtk_source="$root/Release/PeliGames_${version}_GTK-source.tar.xz"
[[ -f "$gtk_source" ]] || { echo 'Missing GTK corresponding source. Rebuild the AppImage with compilar.sh.' >&2; exit 1; }
packages+=("$gtk_source")
# Refuse overwriting an existing version: every published update gets a new version.
if gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
  echo "Release $tag already exists. Increase the version before publishing." >&2; exit 1
fi
gh release create "$tag" "${packages[@]}" --repo "$repo" --target "$(git -C "$root" rev-parse HEAD)" --title "PeliGames $version" --notes-file "$notes" --draft --prerelease
# The release becomes visible to the updater only after every package was uploaded.
gh release edit "$tag" --repo "$repo" --draft=false --prerelease --latest

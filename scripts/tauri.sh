#!/usr/bin/env bash
set -euo pipefail
# Use the timezone passed by the host; otherwise retain the system timezone.

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

# Record this build's start so an AppImage-only build does not copy older DEB/RPM files.
marker=$(mktemp)
layout_config=
trap 'rm -f "$marker"; if [[ -n "$layout_config" ]]; then rm -f "$layout_config"; fi' EXIT

case "${1:-}" in
  build|bundle) ;;
  *) bun "$root/node_modules/@tauri-apps/cli/tauri.js" "$@"; exit 0 ;;
esac

# Tauri requires native icon formats; derive them from the vector source before building.
bun run icons

profile=release
build_target=
bundle_selection=all
previous=
for argument in "$@"; do
  if [[ "$previous" == --target ]]; then build_target=$argument; fi
  if [[ "$previous" == --bundles ]]; then bundle_selection=$argument; fi
  case "$argument" in
    --debug) profile=debug ;;
    --target=*) build_target=${argument#--target=} ;;
    --bundles=*) bundle_selection=${argument#--bundles=} ;;
    --no-bundle) bundle_selection=none ;;
  esac
  previous=$argument
done

# Cargo resolves custom target directories configured through the environment or .cargo.
target_root=$(cargo metadata --offline --no-deps --format-version 1 --manifest-path "$root/src-tauri/Cargo.toml" | bun -e 'let input = ""; for await (const chunk of process.stdin) input += chunk; console.log(JSON.parse(input).target_directory)')
output="$target_root/${build_target:+$build_target/}$profile"
gtk_directory=
gtk_source_name=
# Resolve the compiled executable for release/debug and custom Cargo target directories.
# Tauri patches its bundle type before reading these files for each DEB/RPM bundle.
if [[ "$(uname -s)" == Linux && ( -z "$build_target" || "$build_target" == *linux* ) ]]; then
  if [[ ",$bundle_selection," == *,appimage,* || ",$bundle_selection," == *,all,* ]]; then
    gtk_directory="$target_root/peligames-gtk3-wayland"
    version=$(bun -e 'console.log((await Bun.file(process.argv[1]).json()).version)' "$root/src-tauri/tauri.conf.json")
    gtk_source_name="PeliGames_${version}_GTK-source.tar.xz"
    bash "$root/scripts/gtk3-wayland/build.sh" "$gtk_directory"
  fi
  layout_config=$(mktemp /tmp/peligames-package-layout-XXXXXX.json)
  bun -e '
    const [binary, installer, icon, gtk] = process.argv.slice(1);
    const files = {"/opt/PeliGames/peligames": binary, "/opt/PeliGames/pelinstall": installer, "/usr/share/icons/hicolor/scalable/apps/peligames.svg": icon};
    const appimage = {"usr/bin/pelinstall": installer, "usr/share/icons/hicolor/scalable/apps/peligames.svg": icon};
    if (gtk) {
      appimage["usr/lib/libgdk-3.so.0"] = `${gtk}/libgdk-3.so.0`;
      for (const notice of ["COPYING", "copyright", "README.md"]) appimage[`usr/share/doc/peligames-gtk3/${notice}`] = `${gtk}/${notice}`;
    }
    console.log(JSON.stringify({bundle:{linux:{deb:{files},rpm:{files},appimage:{files:appimage}}}}));
  ' "$output/peligames" "$output/pelinstall" "$root/public/peligames.svg" "$gtk_directory" > "$layout_config"
  # Preserve linuxdeploy diagnostics in the build log instead of a generic failure.
  bun "$root/node_modules/@tauri-apps/cli/tauri.js" "$@" --verbose --config "$layout_config"
else
  bun "$root/node_modules/@tauri-apps/cli/tauri.js" "$@"
fi
# Tauri derives peli-games from the display name; normalize the DEB package ID.
if [[ -d "$output/bundle/deb" ]]; then
  while IFS= read -r -d '' package; do
    bash "$root/scripts/deb-package-name.sh" "$package"
  done < <(find "$output/bundle/deb" -maxdepth 1 -type f -newer "$marker" -name '*.deb' -print0)
fi
# Rewrite RPM's reserved main binary entry while keeping package ownership and dependencies.
if [[ -d "$output/bundle/rpm" ]]; then
  while IFS= read -r -d '' package; do
    if command -v rpmbuild >/dev/null && command -v rpm2cpio >/dev/null && command -v cpio >/dev/null; then
      bash "$root/scripts/rpm-opt-layout.sh" "$package"
    elif command -v distrobox-host-exec >/dev/null; then
      distrobox-host-exec bash "$root/scripts/rpm-opt-layout.sh" "$package"
    else
      echo 'Para gerar RPM em /opt, instale rpm, rpmbuild e cpio.' >&2
      exit 1
    fi
  done < <(find "$output/bundle/rpm" -maxdepth 1 -type f -newer "$marker" -name '*.rpm' -print0)
fi
mkdir -p "$root/Release"

# Replace files atomically, including an executable that is currently running.
copy_to_release() {
  local source=$1 temporary name
  name=${2:-$(basename "$source")}
  # The AppImage keeps one name across versions (PeliGames-x86_64.AppImage): updates
  # replace it in place, and the name still carries the architecture old updaters need.
  case "$name" in
    *.AppImage) name=PeliGames-x86_64.AppImage ;;
    *.AppImage.sig) name=PeliGames-x86_64.AppImage.sig ;;
  esac
  temporary=$(mktemp "$root/Release/.copy-XXXXXX")
  if ! cp -p "$source" "$temporary"; then rm -f "$temporary"; return 1; fi
  if ! mv -f "$temporary" "$root/Release/$name"; then rm -f "$temporary"; return 1; fi
  echo "Copiado para Release/: $name"
}

if [[ -d "$output/bundle" ]]; then
  while IFS= read -r -d '' package; do
    if [[ "$package" == *.AppImage && -n "$gtk_directory" ]]; then
      bash "$root/scripts/gtk3-wayland/verify.sh" "$package" "$gtk_directory/libgdk-3.so.0"
      copy_to_release "$gtk_directory/PeliGames-GTK-source.tar.xz" "$gtk_source_name"
    fi
    copy_to_release "$package"
  done < <(find "$output/bundle" -maxdepth 2 -type f -newer "$marker" \( -name '*.AppImage' -o -name '*.AppImage.sig' -o -name '*.deb' -o -name '*.rpm' \) -print0)
fi
if [[ -f "$output/peligames" && "$output/peligames" -nt "$marker" ]]; then
  copy_to_release "$output/peligames"
fi

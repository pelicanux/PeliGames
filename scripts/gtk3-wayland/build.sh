#!/usr/bin/env bash
# Build only the AppImage's GDK, retaining Ubuntu's security/compatibility patches.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
output=${1:?Usage: build.sh /tmp/output-directory}
version=3.24.41-4ubuntu1.3
for command in dpkg-query dpkg dpkg-source dpkg-deb curl sha256sum patch tar xz cc bun findmnt readelf; do
  command -v "$command" >/dev/null || { echo "Dependência de compilação ausente: $command" >&2; exit 1; }
done
installed=$(dpkg-query -W -f='${source:Version}' libgtk-3-0t64)
[[ $installed == "$version" && $(dpkg --print-architecture) == amd64 ]] || {
  echo "O AppImage corrigido requer GTK $version / amd64. Use o contêiner Ubuntu 24 de compilar.sh (GTK encontrado: $installed)." >&2
  exit 1
}
mkdir -p "$output"
output=$(cd "$output" && pwd)
[[ $(findmnt -no FSTYPE -T "$output" | sort -u) == tmpfs ]] || {
  echo 'A biblioteca gráfica precisa ser compilada em RAM (/tmp).' >&2; exit 1;
}
jobs=${CARGO_BUILD_JOBS:-12}
[[ $jobs =~ ^[1-9][0-9]*$ ]] || { echo 'CARGO_BUILD_JOBS precisa ser um inteiro positivo.' >&2; exit 2; }
(( jobs <= 12 )) || jobs=12
work=$(mktemp -d "$output/.build-XXXXXX")
trap 'rm -rf "$work"' EXIT
downloads="$output/downloads"
mkdir -p "$downloads"
fetch() {
  local url=$1 hash=$2 path=$3
  if [[ ! -f "$path" ]] || ! printf '%s  %s\n' "$hash" "$path" | sha256sum -c --status; then
    curl --fail --location --retry 2 --connect-timeout 20 --max-time 900 "$url" -o "$work/download.part"
    printf '%s  %s\n' "$hash" "$work/download.part" | sha256sum -c --status || {
      echo "Falha na integridade de $(basename "$path")." >&2; return 1;
    }
    mv -f "$work/download.part" "$path"
  fi
}
archive=https://archive.ubuntu.com/ubuntu/pool/main
tools_archive=https://archive.ubuntu.com/ubuntu/pool/universe
fetch "$archive/g/gtk+3.0/gtk+3.0_${version}.dsc" \
  b7ea9dda7ffd3f01f97d7ee673f130a82db5b346ade2d003e609e27886b35269 "$downloads/gtk+3.0_${version}.dsc"
fetch "$archive/g/gtk+3.0/gtk+3.0_3.24.41.orig.tar.xz" \
  47da61487af3087a94bc49296fd025ca0bc02f96ef06c556e7c8988bd651b6fa "$downloads/gtk+3.0_3.24.41.orig.tar.xz"
fetch "$archive/g/gtk+3.0/gtk+3.0_${version}.debian.tar.xz" \
  1a82c70d722296f15af0a00a737a0c552580c6bf3f527f9ef7e66f192fe1831c "$downloads/gtk+3.0_${version}.debian.tar.xz"

# Build tools stay in the temporary directory; nothing is installed on the host.
tools="$work/tools"
mkdir -p "$tools"
if ! command -v meson >/dev/null; then
  file=meson_1.3.2-1ubuntu1_all.deb
  fetch "$tools_archive/m/meson/$file" 5a9ab497255cc51f97866c359fc5cc75a1be4890ad376e471ca45d76bbea4803 "$downloads/$file"
  dpkg-deb -x "$downloads/$file" "$tools"
  export PYTHONPATH="$tools/usr/lib/python3/dist-packages${PYTHONPATH:+:$PYTHONPATH}"
fi
if ! command -v ninja >/dev/null; then
  file=ninja-build_1.11.1-2_amd64.deb
  fetch "$tools_archive/n/ninja-build/$file" 6a17f76a0f75586f1292f6c61a9b07fd0dec963baf6a102b17673f601c6b323a "$downloads/$file"
  dpkg-deb -x "$downloads/$file" "$tools"
fi
export PATH="$tools/usr/bin:$PATH"
source_dir="$work/gtk-source"
build="$work/build"
dpkg-source --no-check -x "$downloads/gtk+3.0_${version}.dsc" "$source_dir"
patch --batch --fuzz=0 -d "$source_dir" -p1 < "$here/frame-clock.patch"
nice -n 19 ionice -c3 meson setup "$build" "$source_dir" --buildtype=release \
  -Dgtk_doc=false -Dman=false -Dintrospection=false -Ddemos=false -Dexamples=false \
  -Dtests=false -Dinstalled_tests=false -Dprint_backends=file -Dcolord=no \
  -Dcloudproviders=false -Dtracker3=false -Dprofiler=false \
  -Dx11_backend=true -Dwayland_backend=true -Dbroadway_backend=true
# Build the shared library target, not GTK widgets, applications or tests.
target=$(bun -e 'const targets=await Bun.file(process.argv[1]).json(); const target=targets.find(t=>t.name==="gdk-3" && t.type==="shared library"); if(!target) throw new Error("Alvo GDK ausente no Meson"); console.log(target.filename[0]);' "$build/meson-info/intro-targets.json")
[[ "$target" == "$build/"* ]] || { echo 'Alvo GDK fora da pasta de compilação.' >&2; exit 1; }
nice -n 19 ionice -c3 ninja -C "$build" -j "$jobs" "${target#"$build/"}"
# Do not retain absolute temporary build paths in the ELF's library lookup.
if LC_ALL=C readelf -d "$target" | grep -Eq '\((RPATH|RUNPATH)\)'; then
  command -v patchelf >/dev/null || { echo 'Instale patchelf no contêiner de compilação.' >&2; exit 1; }
  patchelf --remove-rpath "$target"
fi
install -m755 "$target" "$output/libgdk-3.so.0"
install -m644 "$source_dir/COPYING" "$output/COPYING"
install -m644 "$source_dir/debian/copyright" "$output/copyright"
install -m644 "$here/README.md" "$output/README.md"

# Corresponding source travels alongside the AppImage, as a separate release asset.
sources="$work/PeliGames-GTK-source"
mkdir -p "$sources"
cp "$downloads/gtk+3.0_${version}.dsc" "$downloads/gtk+3.0_3.24.41.orig.tar.xz" \
  "$downloads/gtk+3.0_${version}.debian.tar.xz" "$here/frame-clock.patch" \
  "$here/build.sh" "$here/README.md" "$source_dir/COPYING" "$source_dir/debian/copyright" "$sources/"
tar -C "$work" -cf - PeliGames-GTK-source | xz -T "$jobs" > "$output/PeliGames-GTK-source.tar.xz"
printf 'GDK corrigido preparado: %s\n' "$output/libgdk-3.so.0"

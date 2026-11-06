#!/usr/bin/env bash
# Build-time only. The resulting verifier needs libc, with crypto/zlib linked in.
set -euo pipefail
export PATH="${BUN_INSTALL:-$HOME/.bun}/bin:$PATH"
root=$(cd "$(dirname "$0")/.." && pwd)
output=${1:?Usage: installer-verifier.sh OUTPUT_DIRECTORY}
manifest="$root/scripts/installer-verifier.json"
mkdir -p "$output"
output=$(cd "$output" && pwd)
exec 9>"$output/.build.lock"
flock 9
architecture=$(uname -m)
case "$architecture" in x86_64|aarch64) ;; *) echo 'Arquitetura do verificador não suportada.' >&2; exit 1 ;; esac
read -r version source_url source_hash cmake_version cmake_hash < <(
  bun -e 'const x=await Bun.file(process.argv[1]).json(); console.log(x.version,x.source.url,x.source.sha256,x.cmake.version,x.cmake[process.argv[2]])' "$manifest" "$architecture"
)
cache="${XDG_CACHE_HOME:-$HOME/.cache}/PeliGames-build/installer-verifier"
mkdir -p "$cache"
fetch() {
  local url=$1 destination=$2 expected=$3 temporary
  if [[ -f "$destination" ]] && printf '%s  %s\n' "$expected" "$destination" | sha256sum --check --status; then return; fi
  temporary=$(mktemp "$cache/.download-XXXXXX")
  if ! curl --fail --location --proto '=https' --proto-redir '=https' --retry 2 --connect-timeout 20 --max-time 300 "$url" -o "$temporary" ||
     ! printf '%s  %s\n' "$expected" "$temporary" | sha256sum --check --status; then
    rm -f "$temporary"
    echo 'Não foi possível obter o código do verificador com integridade confirmada.' >&2
    return 1
  fi
  mv -f "$temporary" "$destination"
}
archive="$cache/osslsigncode-$version-source.tar.gz"
fetch "$source_url" "$archive" "$source_hash"
# The standard Ubuntu builder need not install CMake globally or use root.
cmake=${CMAKE:-}
if [[ -z "$cmake" ]]; then cmake=$(command -v cmake || true); fi
if [[ -z "$cmake" ]]; then
  kit="cmake-$cmake_version-linux-$architecture"
  fetch "https://github.com/Kitware/CMake/releases/download/v$cmake_version/$kit.tar.gz" "$cache/$kit.tar.gz" "$cmake_hash"
  if [[ ! -x "$output/$kit/bin/cmake" ]]; then tar -xzf "$cache/$kit.tar.gz" -C "$output"; fi
  cmake="$output/$kit/bin/cmake"
fi
source_dir="$output/osslsigncode-$version"
if [[ ! -d "$source_dir" ]]; then tar -xzf "$archive" -C "$output"; fi
"$cmake" -S "$source_dir" -B "$output/build" -G 'Unix Makefiles' \
  -DCMAKE_BUILD_TYPE=Release -DOPENSSL_USE_STATIC_LIBS=TRUE -DZLIB_USE_STATIC_LIBS=ON \
  -DCMAKE_DISABLE_FIND_PACKAGE_Python3=TRUE -DPANDOC=FALSE \
  -DCMAKE_EXE_LINKER_FLAGS='-static-libgcc -Wl,--as-needed'
"$cmake" --build "$output/build" --target osslsigncode --parallel "${CARGO_BUILD_JOBS:-8}"
# Shipping a shared libcrypto/libssl/libz requirement would reproduce the original bug.
while read -r library; do
  case "$library" in libc.so.6) ;; *) echo "Dependência externa inesperada no verificador: $library" >&2; exit 1 ;; esac
done < <(readelf -d "$output/build/osslsigncode" | sed -n 's/.*NEEDED.*\[\([^]]*\)\].*/\1/p')
"$output/build/osslsigncode" --version
strip "$output/build/osslsigncode"
gzip -n -c "$output/build/osslsigncode" > "$output/osslsigncode.gz.new"
mv -f "$output/osslsigncode.gz.new" "$output/osslsigncode.gz"
# Corresponding source and original notices travel with every package.
mkdir -p "$root/licenses/generated"
cp "$archive" "$root/licenses/generated/osslsigncode-$version-source.tar.gz"
{
  printf 'osslsigncode %s — https://github.com/mtrojnar/osslsigncode\n\n' "$version"
  cat "$source_dir/LICENSE.txt"
  printf '\n\nOpenSSL — https://openssl-library.org/\n\n'
  cat /usr/share/doc/libssl-dev/copyright
  printf '\n\nzlib — https://zlib.net/\n\n'
  cat /usr/share/doc/zlib1g-dev/copyright
} > "$root/licenses/generated/Installer-Verifier-Notices.txt"

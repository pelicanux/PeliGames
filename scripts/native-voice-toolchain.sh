#!/usr/bin/env bash
# Source from Linux build scripts. Respect explicit CXX; otherwise prefer a
# compiler compatible with the hermetic libc++ in LiveKit's WebRTC artifact.
if [[ $(uname -s) == Linux && -z ${CXX:-} ]]; then
  for peli_cxx in clang++-23 clang++-22 clang++-21 clang++; do
    if command -v "$peli_cxx" >/dev/null 2>&1; then
      peli_clang_major=$("$peli_cxx" --version | sed -nE 's/.*clang version ([0-9]+).*/\1/p' | head -n1)
      if [[ ${peli_clang_major:-0} -ge 21 ]]; then
        export CXX="$peli_cxx"
        break
      fi
    fi
  done
  if [[ -z ${CXX:-} ]]; then
    echo 'LiveKit exige Clang 21 ou superior. Instale-o no ambiente de build ou configure CXX.' >&2
    exit 1
  fi
  unset peli_cxx peli_clang_major
fi

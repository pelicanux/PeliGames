#!/usr/bin/env bash
# Local build entry point; uses the existing Ubuntu container without publishing.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
container=${PELIGAMES_BUILD_CONTAINER:-ubuntu-24-dev}
mode=appimage
jobs=${CARGO_BUILD_JOBS:-6}
usage() {
  cat <<'HELP'
Uso: bash compilar.sh [--bin | --all] [--jobs N] [--container NOME]

Sem opções: compila PeliGames, PeliInstall e AppImage.
  --bin        Compila apenas os dois executáveis.
  --all        Compila também DEB e RPM, com hashes de publicação.
  --jobs N     Limita as tarefas de compilação (padrão: 6).
  --container  Usa outro container já preparado (padrão: ubuntu-24-dev).
  --help       Mostra esta ajuda, sem compilar.

Resultados: Release/. Saída detalhada: Release/compilacao.log.
Não envia código nem publica releases no GitHub.
HELP
}
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) mode=bin; shift ;;
    --all) mode=all; shift ;;
    --jobs|--container)
      [[ $# -ge 2 && -n "$2" ]] || { echo "Falta valor para $1." >&2; exit 2; }
      if [[ $1 == --jobs ]]; then jobs=$2; else container=$2; fi
      shift 2 ;;
    --help|-h) usage; exit 0 ;;
    --*) echo "Opção desconhecida: $1" >&2; exit 2 ;;
    *) container=$1; shift ;; # Retain the original positional container argument.
  esac
done
[[ $jobs =~ ^[1-9][0-9]*$ ]] || { echo 'Informe --jobs com um inteiro positivo.' >&2; exit 2; }
for command in podman flock sha256sum; do
  command -v "$command" >/dev/null || { echo "Comando necessário não encontrado: $command" >&2; exit 1; }
done
if [[ $mode == all ]]; then
  for command in bun bsdtar; do
    command -v "$command" >/dev/null || { echo "Para --all, instale $command no host." >&2; exit 1; }
  done
fi
mkdir -p "$root/Release"
exec 9>"$root/Release/.compilacao.lock"
flock -n 9 || { echo 'Já há uma compilação em andamento neste projeto.' >&2; exit 1; }
log="$root/Release/compilacao.log"
[[ ! -f "$log" ]] || mv -f "$log" "$root/Release/compilacao-anterior.log"
: > "$log"
fail() {
  local status=$?
  trap - ERR
  printf '\nA compilação falhou (código %s). Log: %s\n' "$status" "$log" >&2
  tail -n 25 "$log" >&2
  exit "$status"
}
trap fail ERR
state=$(podman inspect --format '{{.State.Running}}' "$container" 2>>"$log") || {
  echo "Container '$container' não encontrado ou inacessível. Veja $log." >&2; exit 1;
}
if [[ "$state" != true ]]; then podman start "$container" >>"$log" 2>&1; fi
host_timezone=${TZ:-}
if [[ -z "$host_timezone" ]]; then
  zone=$(readlink -f /etc/localtime || true)
  if [[ "$zone" == */zoneinfo/* ]]; then host_timezone=${zone#*/zoneinfo/}; fi
fi
if [[ -z "$host_timezone" && -r /etc/timezone ]]; then host_timezone=$(cat /etc/timezone); fi
host_timezone=${host_timezone:-UTC}
script=scripts/build-release.sh
bundles=appimage
if [[ $mode == bin ]]; then script=scripts/build-binaries.sh; fi
if [[ $mode == all ]]; then bundles=deb,rpm,appimage; fi
printf 'Compilando no container %s (%s tarefas). Log: %s\n' "$container" "$jobs" "$log"
podman exec --user "$(id -u):$(id -g)" --workdir "$root" \
  --env "TZ=$host_timezone" --env "CARGO_BUILD_JOBS=$jobs" \
  --env CARGO_INCREMENTAL=0 "$container" bash -c \
  'unset PELIGAMES_OVERLAY_BUNDLE PELIGAMES_LAUNCH_HELPER; exec bash "$@"' \
  peligames-build "$script" "$bundles" >>"$log" 2>&1
if [[ $mode == all ]]; then bash "$root/scripts/release-checksums.sh" >>"$log" 2>&1; fi
artifacts=(peligames pelinstall)
if [[ $mode != bin ]]; then artifacts+=(PeliGames-x86_64.AppImage); fi
(cd "$root/Release" && sha256sum "${artifacts[@]}") > "$root/Release/SHA256SUMS-local.txt"
printf 'Compilação concluída: %s/Release/\n' "$root"
printf '  %s\n' "${artifacts[@]}"

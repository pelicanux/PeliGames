#!/usr/bin/env bash
# Run on the host so the container receives the host's current timezone.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
container=${1:-ubuntu-dev}
host_timezone=${TZ:-}
if [[ -z "$host_timezone" ]]; then
  localtime=$(readlink -f /etc/localtime || true)
  if [[ "$localtime" == */zoneinfo/* ]]; then
    host_timezone=${localtime#*/zoneinfo/}
  elif command -v timedatectl >/dev/null 2>&1; then
    host_timezone=$(timedatectl show --property=Timezone --value 2>/dev/null || true)
  fi
fi
if [[ -z "$host_timezone" && -r /etc/timezone ]]; then
  host_timezone=$(cat /etc/timezone)
fi
if [[ -z "$host_timezone" ]]; then
  echo 'Não foi possível identificar o timezone do host. Informe TZ ao executar este script.' >&2
  exit 1
fi
printf 'Timezone do host: %s\n' "$host_timezone"
exec podman exec --user "$(id -u):$(id -g)" --workdir "$root" \
  --env "TZ=$host_timezone" "$container" bash scripts/build-release.sh

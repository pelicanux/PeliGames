#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
desktop_entry="$project_dir/Release/pelinstall.desktop"
application_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
mime_types=(application/x-ms-dos-executable application/x-msdownload application/x-msi)
previous_defaults=()
for mime_type in "${mime_types[@]}"; do
    previous_defaults+=("$(xdg-mime query default "$mime_type" 2>/dev/null || true)")
done

if [[ ! -x "$project_dir/Release/pelinstall" || ! -f "$desktop_entry" ]]; then
    echo 'Compile o Pelinstall antes de registrar o aplicativo.' >&2
    exit 1
fi
if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$desktop_entry"
fi
mkdir -p -- "$application_dir"
install -m 644 -- "$desktop_entry" "$application_dir/pelinstall.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$application_dir"
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
    kbuildsycoca6
fi
for index in "${!mime_types[@]}"; do
    if [[ -n "${previous_defaults[$index]}" ]]; then
        xdg-mime default "${previous_defaults[$index]}" "${mime_types[$index]}"
    fi
done
echo "Pelinstall registrado em: $application_dir/pelinstall.desktop"
echo 'No gerenciador de arquivos, selecione Pelinstall na lista de Abrir com.'

#!/usr/bin/env bash
# Keep the package ID joined while preserving the PeliGames display name.
set -euo pipefail
package=$(realpath "$1")
work=$(mktemp -d /tmp/peligames-deb-name-XXXXXX)
staged=$(mktemp "$(dirname "$package")/.peligames-deb-XXXXXX")
trap 'rm -rf "$work"; rm -f "$staged"' EXIT
dpkg-deb --raw-extract "$package" "$work"
python3 - "$work/DEBIAN/control" <<'CONTROL'
import sys
from pathlib import Path
path = Path(sys.argv[1])
lines = path.read_text().splitlines()
assert any(line.startswith('Package: ') for line in lines), 'Missing package name'
lines = ['Package: peligames' if line.startswith('Package: ') else line for line in lines]
# A renamed package must replace the old ID instead of conflicting over files.
for field in ('Conflicts', 'Replaces'):
    for index, line in enumerate(lines):
        if line.startswith(field + ':'):
            values = line.split(':', 1)[1].strip()
            if 'peli-games' not in values.split(', '):
                lines[index] = field + ': ' + values + ', peli-games'
            break
    else:
        lines.append(field + ': peli-games')
path.write_text('\n'.join(lines) + '\n')
CONTROL
dpkg-deb --build --root-owner-group "$work" "$staged"
test "$(dpkg-deb --field "$staged" Package)" = peligames
mv -f "$staged" "$package"

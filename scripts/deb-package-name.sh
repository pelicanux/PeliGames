#!/usr/bin/env bash
# Keep the package ID joined while preserving the PeliGames display name.
set -euo pipefail
package=$(realpath "$1")
work=$(mktemp -d /tmp/peligames-deb-name-XXXXXX)
staged=$(mktemp "$(dirname "$package")/.peligames-deb-XXXXXX")
trap 'rm -rf "$work"; rm -f "$staged"' EXIT
dpkg-deb --raw-extract "$package" "$work"
bun -e '
const { readFileSync, writeFileSync } = require("fs");
const path = process.argv[1];
const lines = readFileSync(path, "utf8").split(/\r?\n/).filter((line, index, all) => !(index === all.length - 1 && line === ""));
if (!lines.some(line => line.startsWith("Package: "))) { console.error("Missing package name"); process.exit(1); }
const result = lines.map(line => line.startsWith("Package: ") ? "Package: peligames" : line);
// A renamed package must replace the old ID instead of conflicting over files.
for (const field of ["Conflicts", "Replaces"]) {
  const index = result.findIndex(line => line.startsWith(field + ":"));
  if (index < 0) { result.push(field + ": peli-games"); continue; }
  const values = result[index].split(":").slice(1).join(":").trim();
  if (!values.split(", ").includes("peli-games")) result[index] = field + ": " + values + ", peli-games";
}
writeFileSync(path, result.join("\n") + "\n");
' -- "$work/DEBIAN/control"
dpkg-deb --build --root-owner-group "$work" "$staged"
test "$(dpkg-deb --field "$staged" Package)" = peligames
mv -f "$staged" "$package"

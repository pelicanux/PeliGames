#!/usr/bin/env python3
"""Assemble the public Pages site using an explicit asset allowlist."""
import argparse
import json
import shutil
import re
from html import escape
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--releases", type=Path, help="JSON from gh api repos/pelicanux/PeliGames/releases --paginate --slurp")
args = parser.parse_args()
out = args.output.resolve()
if out.exists():
    raise SystemExit("Choose a new output directory; existing files will not be overwritten.")
release = None
if args.releases:
    data = json.loads(args.releases.read_text())
    releases = [item for page in data for item in page] if data and isinstance(data[0], list) else data
    candidates = [item for item in releases if not item.get("draft") and item.get("published_at")]
    if not candidates:
        raise SystemExit("No public releases found.")
    latest = max(candidates, key=lambda item: item["published_at"])
    release = {"tag": latest["tag_name"], "url": latest["html_url"], "prerelease": latest["prerelease"], "assets": {}}
    for asset in latest["assets"]:
        name = asset["name"]
        kind = "appimage" if name.lower().endswith(".appimage") and "x86_64" in name else "deb" if name.endswith("_amd64.deb") else "rpm" if name.endswith(".x86_64.rpm") else None
        if kind:
            release["assets"][kind] = {"url": asset["browser_download_url"], "size": asset["size"]}
    if set(release["assets"]) != {"appimage", "deb", "rpm"}:
        raise SystemExit("The newest public release is missing a Linux package; publish all packages before deploying.")
assets = {
    "public/peligames.svg": "peligames.svg",
    "public/fonts/jersey-10/Jersey10-Regular.ttf": "Jersey10-Regular.ttf",
    "public/fonts/jersey-10/OFL.txt": "Jersey10-OFL.txt",
    "public/fonts/oxanium/Oxanium-Variable.ttf": "Oxanium-Variable.ttf",
    "public/fonts/oxanium/OFL.txt": "Oxanium-OFL.txt",
    "public/launchers/lutris.svg": "lutris.svg",
    "public/launchers/faugus.svg": "faugus.svg",
    "licenses/upstream/Faugus-Assets/LICENSE": "Faugus-Assets-LICENSE.txt",
    "licenses/upstream/Lutris/LICENSE": "Lutris-LICENSE.txt",
}
for name in ("library", "game-details", "nexus", "dlssnr", "monitoring", "themes"):
    assets[f"public/welcome/{name}.jpg"] = f"{name}.jpg"
for source in assets:
    if not (ROOT / source).is_file():
        raise SystemExit(f"Missing public asset: {source}")
(out / "assets").mkdir(parents=True)
for name in ("index.html", "style.css", "app.js", "credits.html"):
    shutil.copyfile(ROOT / "website" / name, out / name)
for source, name in assets.items():
    shutil.copyfile(ROOT / source, out / "assets" / name)
(out / ".nojekyll").touch()
if release:
    # Keep the generated HTML current even when JavaScript is unavailable.
    html = (out / "index.html").read_text()
    label = escape(release["tag"] + (" experimental" if release["prerelease"] else ""))
    html = re.sub(r'(<span data-release-label>)[^<]+', lambda match: match[1] + label, html)
    html = html.replace("https://github.com/pelicanux/PeliGames/releases/tag/v0.5.1", escape(release["url"], quote=True))
    for kind, asset in release["assets"].items():
        html = re.sub(r'(data-download="' + kind + r'" href=")[^"]+', lambda match: match[1] + escape(asset["url"], quote=True), html)
        html = re.sub(r'(data-size="' + kind + r'">)[^<]+', lambda match: match[1] + str(-(-asset["size"] // 1048576)) + " MB", html)
    (out / "index.html").write_text(html)
    (out / "release.json").write_text(json.dumps(release, ensure_ascii=False) + "\n")
else:
    (out / "release.json").write_text('{}\n')
print(f"Public site: {out} ({sum(p.stat().st_size for p in out.rglob('*') if p.is_file()) / 1048576:.2f} MiB)")

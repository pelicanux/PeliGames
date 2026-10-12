#!/usr/bin/env python3
"""Verifica se as extensões Vortex usadas pelos módulos mudaram desde a revisão fixada.

Lê as revisões de duas fontes: `sources.txt` e as URLs `github.com/<dono>/<repo>/blob/<commit>/…`
citadas em `sources` e `vortex_evidence` dos recursos de `src-tauri/resources/nexus-games`.
Para cada repositório, compara a revisão fixada com a atual pela API pública do GitHub
e relata, por módulo, os arquivos alterados da extensão que ele usa, as extensões novas
e se a mudança parece só cosmética (comentários, versão, changelog).

Só lê: nada é alterado nos módulos. Uso:
    python3 -I tools/vortex-import/check-updates.py [--output relatorio.md]
Com GITHUB_TOKEN no ambiente, o limite da API sobe de 60 para 5000 consultas por hora.
"""
import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
RESOURCES = HERE.parent.parent / "src-tauri" / "resources" / "nexus-games"
BLOB = re.compile(r"https://github\.com/([^/\s\"]+)/([^/\s\"]+)/blob/([0-9a-f]{40})/([^\s\"#?]+)")
API = "https://api.github.com"
# Linhas que não mudam regra: comentários, versão e metadados.
COSMETIC = re.compile(r"^\s*(//|/\*|\*|#|$)|\"version\"\s*:|\bversion\b\s*[:=]|EXTENSION_VERSION|^\s*\"?changelog", re.I)
# Documentação, notas de versão, pacote já montado e imagens não mudam regras.
COSMETIC_FILES = re.compile(r"(^|/)(changelog|readme|license|info\.json|package\.json)$|\.(md|bbcode|txt|zip|png|jpe?g|webp|gif)$", re.I)


def request(path):
    headers = {"User-Agent": "PeliGames-update-check", "Accept": "application/vnd.github+json"}
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    with urllib.request.urlopen(urllib.request.Request(f"{API}{path}", headers=headers), timeout=60) as response:
        return json.load(response)


def extension_folder(repo, path):
    """Pasta da extensão dentro do repositório; vazio quando o repositório é a extensão."""
    parts = path.split("/")
    if repo.lower() == "nexus-mods/vortex" and len(parts) > 2 and parts[0] == "extensions":
        return "/".join(parts[:3] if parts[1] == "games" else parts[:2]) + "/"
    if len(parts) > 1 and re.match(r"^(game|template)-", parts[0]):
        return parts[0] + "/"
    return ""


def modules():
    """(dono/repo, commit) → {pasta: [módulos]} a partir das URLs fixadas nos recursos."""
    pins = defaultdict(lambda: defaultdict(set))
    for file in sorted(RESOURCES.glob("*.json")):
        try:
            data = json.loads(file.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        items = data if isinstance(data, list) else data.get("modules", []) if isinstance(data, dict) else []
        for item in items:
            if not isinstance(item, dict) or "domain" not in item:
                continue
            label = f"{item.get('name', item['domain'])} ({item['domain']})"
            for url in list(item.get("sources", [])) + list(item.get("vortex_evidence", [])):
                match = BLOB.match(url) if isinstance(url, str) else None
                if match:
                    owner, repo, rev, path = match.groups()
                    pins[(f"{owner}/{repo}", rev)][extension_folder(f"{owner}/{repo}", path)].add(label)
    for line in (HERE / "sources.txt").read_text(encoding="utf-8").splitlines():
        fields = line.split()
        if len(fields) == 2 and not line.startswith("#"):
            pins[(fields[0], fields[1])]  # repositório lido pelo importador, mesmo sem módulo citado
    return pins


def changed_files(repo, rev, comparison):
    files = comparison.get("files", [])
    if len(files) < 300:
        return files
    # A comparação lista no máximo 300 arquivos; nesse caso, junta os de cada commit.
    merged = {}
    for commit in comparison.get("commits", []):
        for entry in request(f"/repos/{repo}/commits/{commit['sha']}").get("files", []):
            merged[entry["filename"]] = entry
    return list(merged.values())


def cosmetic(entry):
    if COSMETIC_FILES.search(entry["filename"]):
        return True
    patch = entry.get("patch")
    if patch is None:
        return False
    lines = [line[1:] for line in patch.splitlines() if line[:1] in "+-" and not line.startswith(("+++", "---"))]
    return all(COSMETIC.search(line) for line in lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--output", help="grava o relatório Markdown neste arquivo")
    args = parser.parse_args()
    pins = modules()
    report = ["# Atualizações das extensões Vortex", ""]
    pending = 0
    seen = {}
    for (repo, rev), folders in sorted(pins.items()):
        entry = seen.setdefault((repo.lower(), rev), {"repo": repo, "folders": {}})
        for folder, labels in folders.items():
            entry["folders"].setdefault(folder, set()).update(labels)
    for (_, rev), entry in sorted(seen.items()):
        repo, folders = entry["repo"], entry["folders"]
        try:
            comparison = request(f"/repos/{repo}/compare/{rev}...HEAD")
            # The comparison follows renames; its page names the current repository.
            repo = "/".join(comparison.get("html_url", f"https://github.com/{repo}/").split("/")[3:5])
            files = changed_files(repo, rev, comparison)
        except urllib.error.HTTPError as error:
            if error.code in (403, 429) and error.headers.get("X-RateLimit-Remaining") == "0":
                reset = error.headers.get("X-RateLimit-Reset", "")
                sys.exit(f"Limite da API do GitHub atingido (renova em {reset and __import__('time').strftime('%H:%M', __import__('time').localtime(int(reset)))}). Tente depois ou defina GITHUB_TOKEN.")
            report += [f"## {repo}", "", f"Não foi possível comparar {rev[:7]}: {error}", ""]
            continue
        ahead = comparison.get("ahead_by", 0)
        if ahead == 0:
            report += [f"## {repo} — sem mudanças desde {rev[:7]}", ""]
            continue
        head = comparison["commits"][-1]["sha"] if comparison.get("commits") else "HEAD"
        report += [f"## {repo} — {ahead} commit(s) desde {rev[:7]} (atual {head[:7]})", "",
                   f"Diff completo: https://github.com/{repo}/compare/{rev[:12]}...{head[:12]}", ""]
        by_folder = defaultdict(list)
        for entry in files:
            by_folder[extension_folder(repo, entry["filename"])].append(entry)
        known = set(folders)
        for folder, entries in sorted(by_folder.items()):
            labels = sorted(folders.get(folder, set()) | (folders.get("", set()) if folder else set()))
            kind = "cosmética" if all(cosmetic(e) for e in entries) else "regra"
            names = ", ".join(sorted(e["filename"].removeprefix(folder) for e in entries)[:8])
            more = f" e mais {len(entries) - 8}" if len(entries) > 8 else ""
            if labels:
                pending += kind == "regra"
                report.append(f"- **{folder or repo}** ({kind}) → {'; '.join(labels)}: {names}{more}")
            elif folder and folder not in known and any(e["status"] == "added" for e in entries):
                report.append(f"- **{folder}** (extensão nova, sem módulo): {names}{more}")
        report.append("")
        commits = comparison.get("commits", [])[-5:]
        if commits:
            report += ["Últimos commits:"] + [f"- {c['sha'][:7]} {c['commit']['message'].splitlines()[0]}" for c in commits] + [""]
    report += ["---", f"Mudanças de regra em módulos existentes: {pending}.",
               "Módulos das famílias geradas (`*-vortex.json`): regenere com `tools/vortex-import/run.sh --emit` na revisão nova e compare.",
               "Módulos feitos à mão: revise o diff da extensão, ajuste o módulo, suba a versão e atualize `support-audit.json` e `sources.txt`."]
    text = "\n".join(report) + "\n"
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8")
    sys.stdout.write(text)


if __name__ == "__main__":
    main()

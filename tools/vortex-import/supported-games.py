#!/usr/bin/env python3
"""Gera docs/supported-games.md: os jogos com módulo no PeliGames e quanto do suporte está concluído.

Lê os módulos e as auditorias de `src-tauri/resources/nexus-games`. Uso:
    python3 -I tools/vortex-import/supported-games.py
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
RESOURCES = ROOT / "src-tauri" / "resources" / "nexus-games"
OUTPUT = ROOT / "docs" / "supported-games.md"
MODULES = ["catalog.json", "palworld.json", "valheim.json", "finalfantasy12.json", "crimsondesert.json",
           "unreal-vortex.json", "unity-vortex.json", "re_engine-vortex.json"]
AUDITS = ["support-audit.json", "unreal-vortex-audit.json", "unity-vortex-audit.json", "re_engine-vortex-audit.json"]
FAMILIES = {"unreal-vortex.json": "Unreal", "unity-vortex.json": "Unity", "re_engine-vortex.json": "RE Engine"}
# Notas que registram recursos da extensão Vortex ainda não feitos no módulo.
GAP = re.compile(r"não (é|são) (feit|aplicad|instalad|reconhecid|recriad|convertid|mesclad|suportad)|ficam? de fora|Fora:"
                 r"|não implementad|exige[m]? (um )?(processamento|módulo|patching|instalador) especializad", re.I)
LEVELS = [
    (100, "Verificado em jogo real: instalado e aberto com mods pelo PeliGames."),
    (90, "Paridade com a extensão Vortex: módulo feito à mão, comparado com o código da extensão e testado com pacotes de exemplo, sem lacunas registradas."),
    (80, "Módulo feito à mão e testado com pacotes de exemplo; algum recurso da extensão Vortex ainda não foi feito (descrito nas notas do módulo)."),
    (70, "Gerado do template da família (Unreal, Unity ou RE Engine) e testado com pacotes de exemplo; ainda não revisado em jogo."),
    (60, "Módulo disponível, mas sem suporte Vortex confirmado para o jogo."),
]


def bar(percent):
    filled = percent // 10
    return "🟩" * filled + "⬜" * (10 - filled)


def origin(module, family):
    urls = " ".join(module.get("sources", []))
    if family:
        return f"Template {family} (ChemBoy)"
    if "github.com/Nexus-Mods/" in urls:
        return "Extensão oficial do Vortex"
    if re.search(r"github\.com/(?!Nexus-Mods/)", urls):
        return "Extensão da comunidade"
    return "Regras próprias"


def main():
    audits = {}
    for name in AUDITS:
        for audit in json.loads((RESOURCES / name).read_text(encoding="utf-8"))["modules"]:
            audits[(audit["domain"], audit["name"])] = audit
    rows = []
    for name in MODULES:
        data = json.loads((RESOURCES / name).read_text(encoding="utf-8"))
        for module in data if isinstance(data, list) else [data]:
            audit = audits.get((module["domain"], module["name"]), {})
            family = FAMILIES.get(name)
            if audit.get("runtime_verified"):
                percent = 100
            elif audit.get("vortex_status", "confirmed") != "confirmed" or module.get("vortex_supported") is False:
                percent = 60
            elif family:
                percent = 70
            elif any(GAP.search(note) for note in module.get("notes", [])):
                percent = 80
            else:
                percent = 90
            rows.append((module["name"], percent, origin(module, family), module["domain"]))
    rows.sort(key=lambda row: row[0].casefold())
    counts = {level: sum(1 for row in rows if row[1] == level) for level, _ in LEVELS}
    lines = [
        "# Jogos suportados",
        "",
        f"O PeliGames instala mods do Nexus Mods em **{len(rows)} jogos** com módulo próprio, no Linux (Proton ou versão nativa).",
        "A coluna **Concluído** mostra o quanto do suporte está pronto:",
        "",
        "| Concluído | Significado | Jogos |",
        "|---|---|---|",
    ] + [f"| {level}% | {text} | {counts[level]} |" for level, text in LEVELS] + [
        "",
        "Os módulos seguem as regras das extensões do Vortex (oficiais, da comunidade e templates de família). As diferenças de cada jogo estão nas notas dos módulos, em [src-tauri/resources/nexus-games](../src-tauri/resources/nexus-games). Lista gerada por `tools/vortex-import/supported-games.py`.",
        "",
        "| Jogo | Concluído | Base das regras | Nexus Mods |",
        "|---|---|---|---|",
    ] + [f"| {name} | {bar(percent)} {percent}% | {source} | [{domain}](https://www.nexusmods.com/{domain}) |"
         for name, percent, source, domain in rows]
    OUTPUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{OUTPUT.relative_to(ROOT)}: {len(rows)} jogos")


if __name__ == "__main__":
    main()

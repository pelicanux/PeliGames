#!/usr/bin/env bun
// Gera docs/supported-games.md: os jogos com módulo no PeliGames e quanto do suporte está concluído.
//
// Lê os módulos e as auditorias de `src-tauri/resources/nexus-games`. Uso:
//     bun tools/vortex-import/supported-games.ts
import { readFileSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = join(import.meta.dir, "..", "..");
const RESOURCES = join(ROOT, "src-tauri", "resources", "nexus-games");
const OUTPUT = join(ROOT, "docs", "supported-games.md");
const MODULES = ["catalog.json", "palworld.json", "valheim.json", "finalfantasy12.json", "crimsondesert.json",
  "unreal-vortex.json", "unity-vortex.json", "re_engine-vortex.json"];
const AUDITS = ["support-route-cases.json", "unreal-vortex-route-cases.json", "unity-vortex-route-cases.json", "re_engine-vortex-route-cases.json"];
const FAMILIES: Record<string, string> = { "unreal-vortex.json": "Unreal", "unity-vortex.json": "Unity", "re_engine-vortex.json": "RE Engine" };
// Notas que registram recursos da extensão Vortex ainda não feitos no módulo.
const GAP = /não (é|são) (feit|aplicad|instalad|reconhecid|recriad|convertid|mesclad|suportad)|ficam? de fora|Fora:|não implementad|exige[m]? (um )?(processamento|módulo|patching|instalador) especializad/i;
const LEVELS: [number, string][] = [
  [100, "Verificado em jogo real: instalado e aberto com mods pelo PeliGames."],
  [90, "Paridade com a extensão Vortex: módulo feito à mão, comparado com o código da extensão e testado com pacotes de exemplo, sem lacunas registradas."],
  [80, "Módulo feito à mão e testado com pacotes de exemplo; algum recurso da extensão Vortex ainda não foi feito (descrito nas notas do módulo)."],
  [70, "Gerado do template da família (Unreal, Unity ou RE Engine) e testado com pacotes de exemplo; ainda não revisado em jogo."],
  [60, "Módulo disponível, mas sem suporte Vortex confirmado para o jogo."],
];

type Module = { name: string; domain: string; sources?: string[]; notes?: string[]; vortex_supported?: boolean };
const load = (name: string) => JSON.parse(readFileSync(join(RESOURCES, name), "utf8"));
const bar = (percent: number) => "🟩".repeat(Math.floor(percent / 10)) + "⬜".repeat(10 - Math.floor(percent / 10));

function origin(module: Module, family?: string) {
  const urls = (module.sources ?? []).join(" ");
  if (family) return `Template ${family} (ChemBoy)`;
  if (urls.includes("github.com/Nexus-Mods/")) return "Extensão oficial do Vortex";
  if (/github\.com\/(?!Nexus-Mods\/)/.test(urls)) return "Extensão da comunidade";
  return "Regras próprias";
}

const audits = new Map<string, Record<string, unknown>>();
for (const name of AUDITS) for (const audit of load(name).modules) audits.set(`${audit.domain}\n${audit.name}`, audit);
const rows: [string, number, string, string][] = [];
for (const name of MODULES) {
  const data = load(name);
  for (const module of (Array.isArray(data) ? data : [data]) as Module[]) {
    const audit = audits.get(`${module.domain}\n${module.name}`) ?? {};
    const family = FAMILIES[name];
    let percent: number;
    if (audit.runtime_verified) percent = 100;
    else if ((audit.vortex_status ?? "confirmed") !== "confirmed" || module.vortex_supported === false) percent = 60;
    else if (family) percent = 70;
    else if ((module.notes ?? []).some(note => GAP.test(note))) percent = 80;
    else percent = 90;
    rows.push([module.name, percent, origin(module, family), module.domain]);
  }
}
// Python's casefold order: by lowercase code points, stable for ties.
rows.sort((a, b) => { const x = a[0].toLowerCase(), y = b[0].toLowerCase(); return x < y ? -1 : x > y ? 1 : 0; });
const count = (level: number) => rows.filter(row => row[1] === level).length;
const lines = [
  "# Jogos suportados",
  "",
  `O PeliGames instala mods do Nexus Mods em **${rows.length} jogos** com módulo próprio, no Linux (Proton ou versão nativa).`,
  "A coluna **Concluído** mostra o quanto do suporte está pronto:",
  "",
  "| Concluído | Significado | Jogos |",
  "|---|---|---|",
  ...LEVELS.map(([level, text]) => `| ${level}% | ${text} | ${count(level)} |`),
  "",
  "Os módulos seguem as regras das extensões do Vortex (oficiais, da comunidade e templates de família). As diferenças de cada jogo estão nas notas dos módulos, em [src-tauri/resources/nexus-games](../src-tauri/resources/nexus-games). Lista gerada por `tools/vortex-import/supported-games.ts`.",
  "",
  "| Jogo | Concluído | Base das regras | Nexus Mods |",
  "|---|---|---|---|",
  ...rows.map(([name, percent, source, domain]) => `| ${name} | ${bar(percent)} ${percent}% | ${source} | [${domain}](https://www.nexusmods.com/${domain}) |`),
];
writeFileSync(OUTPUT, lines.join("\n") + "\n");
console.log(`${relative(ROOT, OUTPUT)}: ${rows.length} jogos`);

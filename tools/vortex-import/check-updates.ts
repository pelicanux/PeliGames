#!/usr/bin/env bun
// Verifica se as extensões Vortex usadas pelos módulos mudaram desde a revisão fixada.
//
// Lê as revisões de duas fontes: `sources.txt` e as URLs `github.com/<dono>/<repo>/blob/<commit>/…`
// citadas em `sources` e `vortex_evidence` dos recursos de `src-tauri/resources/nexus-games`.
// Para cada repositório, compara a revisão fixada com a atual pela API pública do GitHub
// e relata, por módulo, os arquivos alterados da extensão que ele usa, as extensões novas
// e se a mudança parece só cosmética (comentários, versão, changelog).
//
// Só lê: nada é alterado nos módulos. Uso:
//     bun tools/vortex-import/check-updates.ts [--output relatorio.md] [--git]
// Com GITHUB_TOKEN no ambiente, o limite da API sobe de 60 para 5000 consultas por hora.
// --git (ou o limite da API atingido) compara pelos repositórios públicos com git, sem
// limite: clones parciais, só histórico e árvores, em ~/.cache/peligames/vortex-git.
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const HERE = import.meta.dir;
const RESOURCES = join(HERE, "..", "..", "src-tauri", "resources", "nexus-games");
const BLOB = /^https:\/\/github\.com\/([^/\s"]+)\/([^/\s"]+)\/blob\/([0-9a-f]{40})\/([^\s"#?]+)/;
const API = "https://api.github.com";
// Linhas que não mudam regra: comentários, versão e metadados.
const COSMETIC = /^\s*(\/\/|\/\*|\*|#|$)|"version"\s*:|\bversion\b\s*[:=]|EXTENSION_VERSION|^\s*"?changelog/i;
// Documentação, notas de versão, pacote já montado e imagens não mudam regras.
const COSMETIC_FILES = /(^|\/)(changelog|readme|license|info\.json|package\.json)$|\.(md|bbcode|txt|zip|png|jpe?g|webp|gif)$/i;

// deno-lint-ignore no-explicit-any
type Json = any;
class HttpError extends Error {
  constructor(readonly status: number, readonly headers: Headers, message: string) { super(message); }
}
async function request(path: string): Promise<Json> {
  const headers: Record<string, string> = { "User-Agent": "PeliGames-update-check", Accept: "application/vnd.github+json" };
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  const response = await fetch(`${API}${path}`, { headers, signal: AbortSignal.timeout(60_000) });
  if (!response.ok) throw new HttpError(response.status, response.headers, `HTTP Error ${response.status}: ${response.statusText}`);
  return response.json();
}

/** Pasta da extensão dentro do repositório; vazio quando o repositório é a extensão. */
export function extensionFolder(repo: string, path: string) {
  const parts = path.split("/");
  if (repo.toLowerCase() === "nexus-mods/vortex" && parts.length > 2 && parts[0] === "extensions")
    return (parts[1] === "games" ? parts.slice(0, 3) : parts.slice(0, 2)).join("/") + "/";
  if (parts.length > 1 && /^(game|template)-/.test(parts[0])) return parts[0] + "/";
  return "";
}

/** "dono/repo\ncommit" → pasta → módulos, a partir das URLs fixadas nos recursos. */
export function modules() {
  const pins = new Map<string, Map<string, Set<string>>>();
  const pin = (repo: string, rev: string) => {
    const key = `${repo}\n${rev}`;
    if (!pins.has(key)) pins.set(key, new Map());
    return pins.get(key)!;
  };
  for (const file of readdirSync(RESOURCES).filter(name => name.endsWith(".json")).sort()) {
    let data: Json;
    try { data = JSON.parse(readFileSync(join(RESOURCES, file), "utf8")); } catch { continue; }
    const items: Json[] = Array.isArray(data) ? data : data && typeof data === "object" ? data.modules ?? [] : [];
    for (const item of items) {
      if (!item || typeof item !== "object" || Array.isArray(item) || !("domain" in item)) continue;
      const label = `${item.name ?? item.domain} (${item.domain})`;
      for (const url of [...(item.sources ?? []), ...(item.vortex_evidence ?? [])]) {
        const match = typeof url === "string" ? BLOB.exec(url) : null;
        if (!match) continue;
        const [, owner, repo, rev, path] = match;
        const folders = pin(`${owner}/${repo}`, rev);
        const folder = extensionFolder(`${owner}/${repo}`, path);
        if (!folders.has(folder)) folders.set(folder, new Set());
        folders.get(folder)!.add(label);
      }
    }
  }
  for (const line of readFileSync(join(HERE, "sources.txt"), "utf8").split(/\r?\n/)) {
    const fields = line.trim().split(/\s+/).filter(Boolean);
    if (fields.length === 2 && !line.startsWith("#")) pin(fields[0], fields[1]); // repositório lido pelo importador, mesmo sem módulo citado
  }
  return pins;
}

const GIT_CACHE = join(process.env.XDG_CACHE_HOME || join(homedir(), ".cache"), "peligames", "vortex-git");
function git(dir: string | null, ...args: string[]): string {
  const result = Bun.spawnSync(["git", ...(dir ? ["-C", dir] : []), ...args], { env: { ...process.env, GIT_TERMINAL_PROMPT: "0" } });
  if (result.exitCode !== 0) throw new Error(result.stderr.toString().trim() || `git ${args[0]} falhou`);
  return result.stdout.toString();
}
const STATUS: Record<string, string> = { A: "added", M: "modified", D: "removed", T: "modified" };
/** Same shape as the API comparison, read from a partial clone (no file contents until a patch is asked). */
function compareWithGit(repo: string, rev: string): Json {
  const url = `https://github.com/${repo}`;
  const head = git(null, "ls-remote", url, "HEAD").split(/\s+/)[0];
  if (!head) throw new Error("repositório sem HEAD");
  if (head === rev) return { ahead_by: 0, html_url: `${url}/`, commits: [], files: [] };
  const dir = join(GIT_CACHE, repo.replace("/", "__"));
  if (!existsSync(dir)) { mkdirSync(GIT_CACHE, { recursive: true }); git(null, "clone", "--quiet", "--bare", "--filter=blob:none", url, dir); }
  else git(dir, "fetch", "--quiet", "--filter=blob:none", "origin", "+HEAD:refs/heads/peligames-head");
  const commits = git(dir, "log", "--format=%H%x00%s", `${rev}..${head}`).trim().split("\n").filter(Boolean).reverse()
    .map(line => { const [sha, message] = line.split("\0"); return { sha, commit: { message } }; });
  const files = git(dir, "diff", "--name-status", "--no-renames", `${rev}..${head}`).trim().split("\n").filter(Boolean).map(line => {
    const [code, filename] = line.split("\t");
    const entry: Json = { filename, status: STATUS[code[0]] ?? "modified" };
    // The patch is read only when the cosmetic check needs it.
    Object.defineProperty(entry, "patch", { enumerable: true, get: () => git(dir, "diff", `${rev}..${head}`, "--", filename) });
    return entry;
  });
  return { ahead_by: commits.length, html_url: `${url}/`, commits, files };
}

async function changedFiles(repo: string, comparison: Json): Promise<Json[]> {
  if (comparison.viaGit) return comparison.files ?? [];
  const files: Json[] = comparison.files ?? [];
  if (files.length < 300) return files;
  // A comparação lista no máximo 300 arquivos; nesse caso, junta os de cada commit.
  const merged = new Map<string, Json>();
  for (const commit of comparison.commits ?? [])
    for (const entry of (await request(`/repos/${repo}/commits/${commit.sha}`)).files ?? []) merged.set(entry.filename, entry);
  return [...merged.values()];
}

function cosmetic(entry: Json) {
  if (COSMETIC_FILES.test(entry.filename)) return true;
  if (entry.patch == null) return false;
  const lines = String(entry.patch).split(/\r?\n/).filter(line => (line[0] === "+" || line[0] === "-") && !line.startsWith("+++") && !line.startsWith("---")).map(line => line.slice(1));
  return lines.every(line => COSMETIC.test(line));
}
const byText = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

async function main() {
  const output = process.argv.indexOf("--output") > -1 ? process.argv[process.argv.indexOf("--output") + 1] : undefined;
  let viaGit = process.argv.includes("--git");
  const pins = modules();
  const report = ["# Atualizações das extensões Vortex", ""];
  let pending = 0;
  // Same repository under different letter cases: one comparison.
  const seen = new Map<string, { repo: string; rev: string; folders: Map<string, Set<string>> }>();
  for (const key of [...pins.keys()].sort(byText)) {
    const [repo, rev] = key.split("\n");
    const id = `${repo.toLowerCase()}\n${rev}`;
    if (!seen.has(id)) seen.set(id, { repo, rev, folders: new Map() });
    const entry = seen.get(id)!;
    for (const [folder, labels] of pins.get(key)!) {
      if (!entry.folders.has(folder)) entry.folders.set(folder, new Set());
      labels.forEach(label => entry.folders.get(folder)!.add(label));
    }
  }
  const order = [...seen.values()].sort((a, b) => byText(a.repo.toLowerCase(), b.repo.toLowerCase()) || byText(a.rev, b.rev));
  for (const { rev, folders, repo: pinned } of order) {
    let repo = pinned, comparison: Json, files: Json[];
    try {
      if (viaGit) comparison = { ...compareWithGit(repo, rev), viaGit: true };
      else comparison = await request(`/repos/${repo}/compare/${rev}...HEAD`);
      // The comparison follows renames; its page names the current repository.
      repo = String(comparison.html_url ?? `https://github.com/${repo}/`).split("/").slice(3, 5).join("/");
      files = await changedFiles(repo, comparison);
    } catch (error) {
      if (error instanceof HttpError && [403, 429].includes(error.status) && error.headers.get("X-RateLimit-Remaining") === "0") {
        const reset = Number(error.headers.get("X-RateLimit-Reset") ?? 0);
        const time = reset ? new Date(reset * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" }) : "";
        console.error(`Limite da API do GitHub atingido (renova em ${time}); seguindo pelo git.`);
        viaGit = true;
        try {
          comparison = { ...compareWithGit(repo, rev), viaGit: true };
          files = await changedFiles(repo, comparison);
        } catch (gitError) {
          report.push(`## ${repo}`, "", `Não foi possível comparar ${rev.slice(0, 7)}: ${gitError instanceof Error ? gitError.message : gitError}`, "");
          continue;
        }
      } else {
        report.push(`## ${repo}`, "", `Não foi possível comparar ${rev.slice(0, 7)}: ${error instanceof Error ? error.message : error}`, "");
        continue;
      }
    }
    const ahead = comparison.ahead_by ?? 0;
    if (ahead === 0) {
      report.push(`## ${repo} — sem mudanças desde ${rev.slice(0, 7)}`, "");
      continue;
    }
    const commits: Json[] = comparison.commits ?? [];
    const head: string = commits.length ? commits[commits.length - 1].sha : "HEAD";
    report.push(`## ${repo} — ${ahead} commit(s) desde ${rev.slice(0, 7)} (atual ${head.slice(0, 7)})`, "",
      `Diff completo: https://github.com/${repo}/compare/${rev.slice(0, 12)}...${head.slice(0, 12)}`, "");
    const byFolder = new Map<string, Json[]>();
    for (const entry of files) {
      const folder = extensionFolder(repo, entry.filename);
      if (!byFolder.has(folder)) byFolder.set(folder, []);
      byFolder.get(folder)!.push(entry);
    }
    for (const folder of [...byFolder.keys()].sort(byText)) {
      const entries = byFolder.get(folder)!;
      const labels = [...new Set([...(folders.get(folder) ?? []), ...(folder ? folders.get("") ?? [] : [])])].sort(byText);
      const kind = entries.every(cosmetic) ? "cosmética" : "regra";
      const names = entries.map(entry => String(entry.filename).startsWith(folder) ? String(entry.filename).slice(folder.length) : entry.filename).sort(byText);
      const shown = names.slice(0, 8).join(", ");
      const more = entries.length > 8 ? ` e mais ${entries.length - 8}` : "";
      if (labels.length) {
        if (kind === "regra") pending++;
        report.push(`- **${folder || repo}** (${kind}) → ${labels.join("; ")}: ${shown}${more}`);
      } else if (folder && !folders.has(folder) && entries.some(entry => entry.status === "added")) {
        report.push(`- **${folder}** (extensão nova, sem módulo): ${shown}${more}`);
      }
    }
    report.push("");
    const recent = commits.slice(-5);
    if (recent.length) report.push("Últimos commits:", ...recent.map(commit => `- ${commit.sha.slice(0, 7)} ${String(commit.commit.message).split(/\r?\n/)[0]}`), "");
  }
  report.push("---", `Mudanças de regra em módulos existentes: ${pending}.`,
    "Módulos das famílias geradas (`*-vortex.json`): regenere com `tools/vortex-import/run.sh --emit` na revisão nova e compare.",
    "Módulos feitos à mão: revise o diff da extensão, ajuste o módulo, suba a versão e atualize `support-route-cases.json` e `sources.txt`.");
  const text = report.join("\n") + "\n";
  if (output) writeFileSync(output, text);
  process.stdout.write(text);
}

if (import.meta.main) await main();

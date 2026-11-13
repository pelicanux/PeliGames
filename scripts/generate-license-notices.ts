#!/usr/bin/env bun
// Preserve original dependency notices in licenses/generated/ (packaged, not versioned).
// Run by scripts/license-notices.sh with cargo metadata from each locked Linux graph.
import { createHash } from "node:crypto";
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative } from "node:path";

const ROOT = join(import.meta.dir, "..");
const PREFIXES = ["license", "licence", "copying", "notice", "copyright"];
type Package = { name: string; version: string; license: string; source: string; repository?: string | null; authors?: string[] };
type Row = Package & { notices: { file: string; sha256: string }[] };

const noticeName = (name: string) => PREFIXES.some(prefix => name.toLowerCase().startsWith(prefix)) || /(?:^|[-_])licen[cs]e(?:[-_.]|$)/i.test(name) || ["OFL.txt", "UFL.txt", "Hack-Regular.txt"].includes(name);
/** Paths compared part by part, as Python sorts them. */
function comparePaths(a: string, b: string) {
  const left = a.split("/"), right = b.split("/");
  for (let i = 0; i < Math.min(left.length, right.length); i++) if (left[i] !== right[i]) return left[i] < right[i] ? -1 : 1;
  return left.length - right.length;
}
function isFile(path: string) {
  try { return statSync(path).isFile(); } catch { return false; }
}
/** Every file below `folder` (symlinked folders are not followed). */
function walk(folder: string, found: string[] = []) {
  for (const name of readdirSync(folder)) {
    const path = join(folder, name);
    const kind = lstatSync(path);
    if (kind.isDirectory()) walk(path, found);
    else found.push(path);
  }
  return found;
}
function noticeFiles(folder: string) {
  return walk(folder).filter(path => isFile(path) && noticeName(basename(path)) && statSync(path).size < 200_000).sort(comparePaths);
}
/** Text as Python reads it: replacement characters and universal newlines. */
const readText = (path: string) => new TextDecoder("utf-8").decode(readFileSync(path)).replace(/\r\n?/g, "\n");
const sha256 = (text: string) => createHash("sha256").update(text, "utf8").digest("hex");

function generate(label: string, packages: [Package, [string, string][]][], destination: string) {
  const documents = new Map<string, string>(), rows: Row[] = [], missing: string[] = [];
  const sections = [`${label} — original third-party notices\n`,
    "Generated from installed, locked dependency versions. Includes transitive and build-time dependencies; this is not a claim that every listed package is shipped in the executable.\n"];
  if (label === "Rust dependencies")
    sections.push("The Source Code Form of each MPL-2.0 crate is available, unmodified, at the crates.io address listed with its version below.\n");
  const ordered = [...packages].sort(([a], [b]) => a.name !== b.name ? (a.name < b.name ? -1 : 1) : a.version < b.version ? -1 : a.version > b.version ? 1 : 0);
  for (const [pkg, files] of ordered) {
    const row: Row = { ...pkg, notices: [] };
    sections.push(`\n${pkg.name} ${pkg.version}\nDeclared license: ${pkg.license}\nSource: ${pkg.source}\n`);
    if (pkg.authors?.length) sections.push("Authors declared by package: " + pkg.authors.join(", ") + "\n");
    for (const [path, filename] of files) {
      const data = readText(path), digest = sha256(data);
      if (!documents.has(digest)) documents.set(digest, data);
      row.notices.push({ file: filename, sha256: digest });
      sections.push(`Original file: ${filename} — document ${digest}\n`);
    }
    if (!files.length) {
      missing.push(pkg.name);
      sections.push("NOTICE TEXT NOT FOUND — manual review required; metadata alone is not a substitute for the license.\n");
    }
    rows.push(row);
  }
  sections.push("\nFULL ORIGINAL DOCUMENTS (identical texts deduplicated by SHA-256)\n");
  for (const digest of [...documents.keys()].sort()) sections.push(`\n===== DOCUMENT ${digest} =====\n${documents.get(digest)}\n`);
  writeFileSync(destination, sections.join(""));
  return [rows, missing] as const;
}

// Standalone add-ons reuse the generator without changing the application's notices.
const metadataPaths: string[] = [];
let rustOnly = false, output = join(ROOT, "licenses/generated");
const args = process.argv.slice(2);
for (let index = 0; index < args.length; index++) {
  const argument = args[index];
  if (argument === "--rust-only") rustOnly = true;
  else if (argument === "--output-dir") {
    const directory = args[++index];
    if (!directory || directory.startsWith("--")) throw new Error("--output-dir requires a directory");
    output = directory;
  } else if (argument.startsWith("--")) throw new Error(`Unknown option: ${argument}`);
  else metadataPaths.push(argument);
}
if (!metadataPaths.length) { console.error("Usage: bun scripts/generate-license-notices.ts [--rust-only] [--output-dir DIRECTORY] <cargo-metadata.json> [additional-metadata.json ...]"); process.exit(2); }
// The bundled PeliRun and in-game overlay have separate locked Cargo graphs.
// Their runtime dependencies need the same notices as the main application.
const merged = new Map<string, any>();
for (const path of metadataPaths) for (const pkg of JSON.parse(readFileSync(path, "utf8")).packages) {
  if (pkg.source) merged.set(`${pkg.name}:${pkg.version}:${pkg.source}`, pkg);
}
const metadata = { packages: [...merged.values()] };
const rust: [Package, [string, string][]][] = [];
for (const pkg of metadata.packages) {
  if (pkg.name === "peligames") continue;
  let folder = dirname(pkg.manifest_path);
  let files: [string, string][] = noticeFiles(folder).map(path => [path, relative(folder, path)]);
  const eguiCrates = ["ecolor", "egui", "emath", "epaint", "epaint_default_fonts"];
  const fallback = join(ROOT, "licenses/upstream", eguiCrates.includes(pkg.name) && pkg.version === "0.31.1" ? "egui" : pkg.name);
  // Include repository-level terms even when a font notice was already present.
  if (existsSync(fallback)) files.push(...noticeFiles(fallback).map(path => [path, relative(ROOT, path)] as [string, string]));
  if (!files.length && pkg.name === "libappindicator-sys") {
    const parent = metadata.packages.find((other: { name: string }) => other.name === "libappindicator");
    folder = dirname(parent.manifest_path);
    files = noticeFiles(folder).map(path => [path, "libappindicator/" + relative(folder, path)]);
  }
  rust.push([{ name: pkg.name, version: pkg.version, license: pkg.license || "UNKNOWN",
    source: `https://crates.io/crates/${pkg.name}/${pkg.version}`, repository: pkg.repository ?? null, authors: pkg.authors ?? [] }, files]);
}
// Only frontend production dependencies and their transitive dependencies.
const pending = rustOnly ? [] : Object.keys(JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")).dependencies);
const seen = new Set<string>(), frontend: [Package, [string, string][]][] = [];
while (pending.length) {
  const name = pending.pop()!;
  if (seen.has(name)) continue;
  seen.add(name);
  const folder = join(ROOT, "node_modules", name);
  const pkg = JSON.parse(readFileSync(join(folder, "package.json"), "utf8"));
  pending.push(...Object.keys(pkg.dependencies ?? {}));
  const files: [string, string][] = readdirSync(folder).sort().map(file => join(folder, file)).filter(path => isFile(path) && noticeName(basename(path))).map(path => [path, basename(path)]);
  // Plugin SPDX summaries do not contain full license terms; API has the same upstream license texts.
  if (name.startsWith("@tauri-apps/") && !files.some(([path]) => basename(path).startsWith("LICENSE-MIT"))) {
    const api = join(ROOT, "node_modules/@tauri-apps/api");
    files.push(...readdirSync(api).filter(file => file.startsWith("LICENSE-")).sort().map(file => [join(api, file), "@tauri-apps/api/" + file] as [string, string]));
  }
  frontend.push([{ name: pkg.name, version: pkg.version, license: pkg.license ?? "UNKNOWN", source: `https://www.npmjs.com/package/${pkg.name}/v/${pkg.version}` }, files]);
}
mkdirSync(output, { recursive: true });
const [r, rm] = generate("Rust dependencies", rust, join(output, "Rust-Dependencies.txt"));
const [f, fm] = rustOnly ? [[], []] : generate("Frontend production dependencies", frontend, join(output, "Frontend-Dependencies.txt"));
console.log(`Avisos de licença: ${r.length} pacotes Rust, ${f.length} da interface.`);
// A package without its license text cannot ship: add the upstream text to licenses/upstream/<name>/.
if (rm.length || fm.length) {
  console.error(`Pacotes sem texto de licença: ${[...rm, ...fm].join(", ")}`);
  process.exit(1);
}

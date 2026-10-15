#!/usr/bin/env bun
// Preserve original dependency notices. Supply cargo metadata from the locked Linux build:
//   cargo metadata --format-version 1 --locked --offline --filter-platform x86_64-unknown-linux-gnu \
//     --manifest-path src-tauri/Cargo.toml > /tmp/metadata.json
//   bun scripts/generate-license-notices.ts /tmp/metadata.json
import { createHash } from "node:crypto";
import { existsSync, lstatSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative } from "node:path";

const ROOT = join(import.meta.dir, "..");
const PREFIXES = ["license", "licence", "copying", "notice", "copyright"];
type Package = { name: string; version: string; license: string; source: string; repository?: string | null; authors?: string[] };
type Row = Package & { notices: { file: string; sha256: string }[] };

const noticeName = (name: string) => PREFIXES.some(prefix => name.toLowerCase().startsWith(prefix));
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
    sections.push("Unmodified source archives for the MPL-2.0 crates are provided under that license in the bundled licenses/sources/ directory and at https://github.com/pelicanux/PeliGames/tree/main/licenses/sources . See manifest.json for versions and Cargo.lock-matching checksums. These archives do not cover the native libraries in the AppImage.\n");
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
/** JSON with non-ASCII characters escaped, as the inventory always used. */
const asciiJson = (value: unknown) => JSON.stringify(value, null, 2).replace(/[^\x00-\x7f]/g, char => "\\u" + char.charCodeAt(0).toString(16).padStart(4, "0"));

const metadataPath = process.argv[2];
if (!metadataPath) { console.error("Usage: bun scripts/generate-license-notices.ts <cargo-metadata.json>"); process.exit(2); }
const metadata = JSON.parse(readFileSync(metadataPath, "utf8"));
const rust: [Package, [string, string][]][] = [];
for (const pkg of metadata.packages) {
  if (pkg.name === "peligames") continue;
  let folder = dirname(pkg.manifest_path);
  let files: [string, string][] = noticeFiles(folder).map(path => [path, relative(folder, path)]);
  if (!files.length) {
    const fallback = join(ROOT, "licenses/upstream", pkg.name);
    files = existsSync(fallback) ? noticeFiles(fallback).map(path => [path, relative(ROOT, path)]) : [];
  }
  if (!files.length && pkg.name === "libappindicator-sys") {
    const parent = metadata.packages.find((other: { name: string }) => other.name === "libappindicator");
    folder = dirname(parent.manifest_path);
    files = noticeFiles(folder).map(path => [path, "libappindicator/" + relative(folder, path)]);
  }
  rust.push([{ name: pkg.name, version: pkg.version, license: pkg.license || "UNKNOWN",
    source: `https://crates.io/crates/${pkg.name}/${pkg.version}`, repository: pkg.repository ?? null, authors: pkg.authors ?? [] }, files]);
}
// Only frontend production dependencies and their transitive dependencies.
const pending = Object.keys(JSON.parse(readFileSync(join(ROOT, "package.json"), "utf8")).dependencies);
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
const output = join(ROOT, "licenses");
const [r, rm] = generate("Rust dependencies", rust, join(output, "Rust-Dependencies.txt"));
const [f, fm] = generate("Frontend production dependencies", frontend, join(output, "Frontend-Dependencies.txt"));
writeFileSync(join(output, "dependency-inventory.json"), asciiJson({ rust: r, frontend: f, missing_notice_texts: [...rm, ...fm] }) + "\n");
console.log(`Rust: ${r.length}; frontend: ${f.length}; missing notice texts: ${JSON.stringify([...rm, ...fm])}`);

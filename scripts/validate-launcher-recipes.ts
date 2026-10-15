#!/usr/bin/env bun
// Offline validation of the catalog, schema subset and cross-file invariants.
//
// The schema evaluator implements precisely the keywords used by
// recipe.schema.json; it is not a general JSON Schema library. This validates
// data, never downloads files or executes installation plans.
// Usage: bun scripts/validate-launcher-recipes.ts
import { createHash } from "node:crypto";
import { lstatSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

export const ROOT = join(import.meta.dir, "..", "installers");
const DOWNLOAD_HOSTS: Record<string, string> = {
  "ubisoft-connect": "ubistatic3-a.akamaihd.net",
  "ea-app": "origin-a.akamaihd.net",
  "battle-net": "www.battle.net",
};
const STEPS = ["preflight", "select_runner", "download_installer", "create_prefix",
  "review_dependencies", "run_installer", "verify_executable", "register_launcher"];

// deno-lint-ignore no-explicit-any
type Json = any;
export class InvalidCatalog extends Error {}
function require(condition: unknown, message: string): asserts condition {
  if (!condition) throw new InvalidCatalog(message);
}

/** JSON.parse refusing duplicate keys (a later key would silently win). */
export function parseStrict(text: string): Json {
  let at = 0;
  const fail = (): never => { throw new InvalidCatalog(`Invalid JSON at ${at}`); };
  const space = () => { while (/\s/.test(text[at] ?? "")) at++; };
  const value = (): Json => {
    space();
    const char = text[at];
    if (char === "{") {
      at++;
      const result: Record<string, Json> = {};
      space();
      if (text[at] === "}") { at++; return result; }
      for (;;) {
        space();
        if (text[at] !== '"') fail();
        const key = string();
        require(!(key in result), `Duplicate JSON key: ${key}`);
        space();
        if (text[at++] !== ":") fail();
        result[key] = value();
        space();
        if (text[at] === ",") { at++; continue; }
        if (text[at++] === "}") return result;
        fail();
      }
    }
    if (char === "[") {
      at++;
      const result: Json[] = [];
      space();
      if (text[at] === "]") { at++; return result; }
      for (;;) {
        result.push(value());
        space();
        if (text[at] === ",") { at++; continue; }
        if (text[at++] === "]") return result;
        fail();
      }
    }
    if (char === '"') return string();
    const literal = /^(?:true|false|null|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?)/.exec(text.slice(at))?.[0];
    if (!literal) fail();
    at += literal!.length;
    return JSON.parse(literal!);
  };
  const string = (): string => {
    const match = /^"(?:[^"\\\u0000-\u001f]|\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4}))*"/.exec(text.slice(at));
    if (!match) fail();
    at += match![0].length;
    return JSON.parse(match![0]);
  };
  const result = value();
  space();
  if (at !== text.length) fail();
  return result;
}
export const read = (path: string) => parseStrict(readFileSync(path, "utf8"));

const TYPES: Record<string, (value: Json) => boolean> = {
  object: value => typeof value === "object" && value !== null && !Array.isArray(value),
  array: Array.isArray,
  string: value => typeof value === "string",
  boolean: value => typeof value === "boolean",
  integer: Number.isInteger,
  null: value => value === null,
};
export function shape(value: Json, schema: Json, location = "$") {
  const allowed = new Set(["type", "const", "enum", "minimum", "minLength", "pattern",
    "properties", "required", "additionalProperties", "items", "$schema", "title"]);
  require(Object.keys(schema).every(key => allowed.has(key)), `${location}: unsupported schema keyword`);
  if ("type" in schema) {
    const names: string[] = Array.isArray(schema.type) ? schema.type : [schema.type];
    require(names.some(name => TYPES[name](value)), `${location}: wrong type`);
  }
  if ("const" in schema) require(typeof value === typeof schema.const && value === schema.const, `${location}: wrong constant`);
  if ("enum" in schema) require(schema.enum.includes(value), `${location}: unknown value ${JSON.stringify(value)}`);
  if (typeof value === "string") {
    require(value.length >= (schema.minLength ?? 0), `${location}: empty value`);
    if ("pattern" in schema) require(new RegExp(schema.pattern, "u").test(value), `${location}: invalid format`);
  }
  if (Number.isInteger(value) && "minimum" in schema) require(value >= schema.minimum, `${location}: below minimum`);
  if (TYPES.object(value)) {
    const properties = schema.properties ?? {};
    require((schema.required ?? []).every((key: string) => key in value), `${location}: missing properties`);
    if (schema.additionalProperties === false) require(Object.keys(value).every(key => key in properties), `${location}: unknown properties`);
    for (const [key, item] of Object.entries(value)) if (key in properties) shape(item, properties[key], `${location}.${key}`);
  }
  if (Array.isArray(value) && "items" in schema) value.forEach((item, index) => shape(item, schema.items, `${location}[${index}]`));
}

function https(url: string) {
  let parsed: URL;
  try { parsed = new URL(url); } catch { throw new InvalidCatalog(`Unsafe URL: ${url}`); }
  require(parsed.protocol === "https:" && parsed.hostname && !parsed.username && !parsed.password
    && !url.includes("#") && (parsed.port === "" || parsed.port === "443"), `Unsafe URL: ${url}`);
  return parsed.hostname;
}

function relative(path: string, glob = false) {
  const parts = path.split("/").filter(part => part !== "" && part !== ".");
  require(parts.length && !path.startsWith("/") && !path.split("/").some(part => part === "." || part === "..")
    && !path.includes("\\") && !path.includes(":") && !path.includes("\0"), `Unsafe path: ${path}`);
  if (glob) require(parts.every(part => ![..."?[]"].some(c => part.includes(c)) && (!part.includes("*") || part === "*")), `Unbounded glob: ${path}`);
  else require(![..."*?[]"].some(c => path.includes(c)), `Unexpected glob: ${path}`);
  return parts;
}

export function validateRecipe(recipe: Json, schema: Json, sourceIds: Set<string>) {
  shape(recipe, schema);
  require(recipe.id in DOWNLOAD_HOSTS, "Launcher outside current catalog scope");
  require(https(recipe.download.url) === DOWNLOAD_HOSTS[recipe.id], "Non-official download host");
  const filename: string = recipe.download.filename;
  require(relative(filename).length === 1 && filename.toLowerCase().endsWith("." + recipe.download.kind), "Invalid installer filename");
  require(JSON.stringify(recipe.steps) === JSON.stringify(STEPS), "Missing or reordered verification steps");
  require(recipe.runners.length, "No runner candidates");
  require(recipe.executable.relative_paths.length, "No executable paths");
  for (const path of [...recipe.executable.relative_paths, ...recipe.executable.relative_globs]) {
    const parts = relative(path, recipe.executable.relative_globs.includes(path));
    require(parts[0] === "drive_c" && path.endsWith(".exe"), "Executable outside prefix or wrong extension");
  }
  for (const candidate of recipe.runners) {
    require(candidate.phases.length && new Set(candidate.phases).size === candidate.phases.length, "Invalid runner phases");
    if (candidate.family.endsWith("proton")) require(candidate.backend === "umu", "Proton outside Steam requires UMU");
  }
  for (const key of ["dependencies", "mitigations", "known_issues"]) {
    const field = key === "dependencies" ? "verb" : "id";
    require(new Set(recipe[key].map((item: Json) => item[field])).size === recipe[key].length, `Duplicate ${key}`);
  }
  const status = recipe.validation;
  if (status.status === "research-only")
    require(!status.installation_tested && !status.launch_tested && !recipe.runners.some((runner: Json) => runner.tested_by_peligames), "Contradictory test status");
  else if (status.status === "installation-tested")
    require(status.installation_tested && !status.launch_tested, "Contradictory test status");
  else require(status.installation_tested && status.launch_tested, "Launch-tested requires installation test");
  const references = (value: Json) => {
    if (TYPES.object(value)) {
      if ("evidence" in value) require(value.evidence?.length && value.evidence.every((id: string) => sourceIds.has(id)), "Missing or unknown evidence");
      Object.values(value).forEach(references);
    } else if (Array.isArray(value)) value.forEach(references);
  };
  references(recipe);
}

export function validateCatalog(root = ROOT) {
  const catalog = read(join(root, "index.json"));
  require(catalog.schema_version === 1 && catalog.minimum_recipe_schema === 1, "Unknown catalog schema");
  require(catalog.status === "experimental", "Catalog needs explicit promotion review");
  const sources: Json[] = catalog.sources;
  const ids = new Set<string>(sources.map(source => source.id));
  require(ids.size === sources.length, "Duplicate sources");
  for (const source of sources) https(source.url);
  for (const check of catalog.preflight) require(check.evidence.every((id: string) => ids.has(id)), "Unknown preflight evidence");
  require(catalog.policy.arbitrary_shell === false && catalog.policy.requires_root === false, "Unsafe catalog policy");
  const schema = read(join(root, "recipe.schema.json"));
  const entries: Json[] = catalog.launchers;
  const scope = Object.keys(DOWNLOAD_HOSTS);
  require(entries.length === scope.length && scope.every(id => entries.some(entry => entry.id === id)), "Wrong catalog scope");
  require(catalog.excluded.includes("gog-galaxy"), "GOG must remain excluded");
  const expected = new Set<string>();
  for (const entry of entries) {
    const file = `recipes/${entry.id}.json`;
    require(entry.path === file, "Invalid catalog path");
    const path = join(root, file);
    require(!lstatSync(path).isSymbolicLink(), "Symlink recipe forbidden");
    require(createHash("sha256").update(readFileSync(path)).digest("hex") === entry.sha256, "Recipe hash mismatch; review and update index");
    const recipe = read(path);
    require(recipe.id === entry.id && recipe.name === entry.name && recipe.recipe_version === entry.recipe_version, "Index and recipe disagree");
    validateRecipe(recipe, schema, ids);
    expected.add(`${entry.id}.json`);
  }
  const present = readdirSync(join(root, "recipes")).filter(name => name.endsWith(".json"));
  require(present.length === expected.size && present.every(name => expected.has(name)), "Unindexed recipe");
  return entries.length;
}

if (import.meta.main) {
  try {
    const count = validateCatalog();
    console.log(`Validated ${count} experimental launcher recipes (offline; no installers executed).`);
  } catch (error) {
    console.error(`Invalid launcher catalog: ${error instanceof Error ? error.message : error}`);
    process.exit(1);
  }
}

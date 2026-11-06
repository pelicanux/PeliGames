import {readdir, readFile, writeFile, mkdir, unlink} from "node:fs/promises";
import {join, resolve} from "node:path";
export const root = resolve(import.meta.dir, "../..");
export const localeRoot = join(root, "src/i18n/locales");
export type Messages = Record<string, string>;
export interface Definition {id: string; nativeName: string; direction: "ltr" | "rtl"; aliases: string[]; published: boolean}
export async function readCatalogs(directory = localeRoot) {
  const result = new Map<string, {definition: Definition; messages: Record<string, Messages>}>();
  for (const entry of (await readdir(directory, {withFileTypes: true})).filter(entry => entry.isDirectory()).sort((a, b) => a.name.localeCompare(b.name))) {
    const folder = join(directory, entry.name);
    const definition = {...JSON.parse(await readFile(join(folder, "language.json"), "utf8")), id: entry.name} as Definition;
    const messages: Record<string, Messages> = {};
    for (const file of (await readdir(folder)).filter(file => file.endsWith(".json") && file !== "language.json").sort()) {
      messages[file.slice(0, -5)] = JSON.parse(await readFile(join(folder, file), "utf8"));
    }
    result.set(entry.name, {definition, messages});
  }
  return result;
}
export function definitionErrors(definition: Definition): string[] {
  const errors: string[] = [];
  try {if (Intl.getCanonicalLocales(definition.id)[0] !== definition.id) errors.push("use a canonical BCP 47 locale tag");} catch {errors.push("invalid locale tag");}
  if (typeof definition.nativeName !== "string" || !definition.nativeName.trim() || !["ltr", "rtl"].includes(definition.direction) || typeof definition.published !== "boolean") errors.push("invalid language metadata");
  if (!Array.isArray(definition.aliases) || definition.aliases.some(alias => {
    try {return typeof alias !== "string" || !Intl.getCanonicalLocales(alias)[0];} catch {return true;}
  })) errors.push("invalid language aliases");
  return errors;
}
/** Accept old {name} tokens until their call sites migrate, as well as i18next {{name}}. */
export function placeholders(text: string) {
  return [...new Set([...text.matchAll(/\{\{\s*(-?\s*[\w.]+)(?:,[^}]+)?\s*\}\}|(?<!\{)\{([a-zA-Z]\w*)\}(?!\})/g)].map(match => (match[1] || match[2]).trim()))].sort();
}
const pluralSuffix = /_(zero|one|two|few|many|other)$/;
function pluralFamilies(base: Messages) {return Object.keys(base).filter(key => key.endsWith("_other") && key.replace(/_other$/, "_one") in base).map(key => key.slice(0, -6));}
export function checkMessages(base: Messages, candidate: Messages, label: string, partial = false, locale = "en"): string[] {
  const errors: string[] = [], families = pluralFamilies(base);
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) return [`${label}: expected a message object`];
  const categories = new Intl.PluralRules(locale).resolvedOptions().pluralCategories;
  for (const [key, value] of Object.entries(candidate)) {
    const stem = key.replace(pluralSuffix, ""), plural = families.includes(stem) && pluralSuffix.test(key);
    if (!(key in base) && !plural) errors.push(`${label}: unknown key ${key}`);
    if (typeof value !== "string" || !value.trim()) {errors.push(`${label}: empty or non-string ${key}`); continue;}
    const reference = base[key] ?? (plural ? base[`${stem}_other`] : undefined);
    if (reference && JSON.stringify(placeholders(reference)) !== JSON.stringify(placeholders(value))) errors.push(`${label}: variables differ in ${key}`);
  }
  if (!partial) {
    for (const key of Object.keys(base)) {
      if (families.includes(key.replace(pluralSuffix, ""))) continue;
      if (!(key in candidate)) errors.push(`${label}: missing ${key}`);
    }
    for (const family of families) for (const category of categories) if (!(`${family}_${category}` in candidate)) errors.push(`${label}: missing ${family}_${category}`);
  }
  return errors;
}
export async function validateCatalogs(directory = localeRoot) {
  const catalogs = await readCatalogs(directory), base = catalogs.get("en");
  if (!base?.definition.published) throw Error("A published English reference catalogue is required");
  const errors: string[] = [], aliases = new Map<string, string>();
  for (const [id, {definition, messages}] of catalogs) {
    const invalid = definitionErrors(definition); errors.push(...invalid.map(error => `${id}: ${error}`));
    if (invalid.length) continue;
    for (const tag of [id, ...definition.aliases]) {
      const canonical = Intl.getCanonicalLocales(tag)[0];
      if (aliases.has(canonical) && aliases.get(canonical) !== id) errors.push(`${id}: ambiguous alias ${tag}`);
      aliases.set(canonical, id);
    }
    for (const [namespace, reference] of Object.entries(base.messages)) {
      if (!/^[A-Za-z][A-Za-z0-9-]*$/.test(namespace)) errors.push(`Invalid namespace: ${namespace}`);
      if (!messages[namespace]) {if (definition.published) errors.push(`${id}: missing namespace ${namespace}`); continue;}
      errors.push(...checkMessages(reference, messages[namespace], `${id}/${namespace}`, !definition.published, id));
    }
    for (const ns of Object.keys(messages)) if (!(ns in base.messages)) errors.push(`${id}: unknown namespace ${ns}`);
  }
  if (errors.length) throw Error(errors.join("\n"));
  return catalogs;
}
export async function generate(projectRoot = root) {
  const catalogs = await validateCatalogs(join(projectRoot, "src/i18n/locales")), base = catalogs.get("en")!, namespaces = Object.keys(base.messages).sort();
  const directory = join(projectRoot, "src/i18n"), packs = join(directory, "localePacks");
  const imports = namespaces.map((ns, i) => `import n${i} from "./locales/en/${ns}.json";`).join("\n");
  const shape = namespaces.map((ns, i) => `${JSON.stringify(ns)}: n${i}`).join(",\n");
  await mkdir(packs, {recursive: true});
  for (const file of await readdir(packs)) if (file.endsWith(".generated.ts") && !catalogs.has(file.replace(/\.generated\.ts$/, ""))) await unlink(join(packs, file));
  for (const [id, {messages}] of catalogs) {
    if (id === "en") continue;
    const entries = Object.keys(messages).sort();
    await writeFile(join(packs, `${id}.generated.ts`), "// Generated by bun run i18n:generate. Edit the JSON catalogues.\n" + entries.map((ns, i) => `import n${i} from "../locales/${id}/${ns}.json";`).join("\n") + "\nexport default {\n" + entries.map((ns, i) => `${JSON.stringify(ns)}: n${i}`).join(",\n") + "\n};\n");
  }
  await writeFile(join(directory, "catalog.generated.ts"), `// Generated by bun run i18n:generate. Edit the JSON catalogues.\nimport type {LanguageDefinition} from "./preference";\n${imports}\nexport const baseResources = {${shape}};\nexport type Catalog = typeof baseResources;\nexport type Namespace = keyof Catalog;\nexport const languages: readonly LanguageDefinition[] = ${JSON.stringify([...catalogs.values()].map(value => value.definition), null, 2)};\nexport const languageLoaders: Record<string, () => Promise<{default: Partial<Record<Namespace, Record<string,string>>>}>> = {\n${[...catalogs.keys()].filter(id => id !== "en").map(id => `${JSON.stringify(id)}: () => import("./localePacks/${id}.generated")`).join(",\n")}\n};\n`);
  return {languages: catalogs.size, namespaces: namespaces.length, messages: Object.values(base.messages).reduce((sum, value) => sum + Object.keys(value).length, 0)};
}

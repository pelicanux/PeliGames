import {readFile, writeFile, mkdir, readdir} from "node:fs/promises";
import {resolve, join} from "node:path";
import {validateCatalogs, checkMessages, definitionErrors, root, type Definition, type Messages} from "./catalogs";
function locale(tag: string) {if (Intl.getCanonicalLocales(tag)[0] !== tag) throw Error("Use a canonical BCP 47 locale tag");}
function catalogueDirectory(projectRoot: string) {return join(projectRoot, "src/i18n/locales");}
function external(folder: string, projectRoot: string) {
  const path = resolve(folder), source = catalogueDirectory(projectRoot);
  if (path === source || path.startsWith(source + "/")) throw Error("Use a kit folder outside src/i18n/locales");
  return path;
}
export async function exportKit(tag: string, output: string, projectRoot = root) {
  locale(tag);
  const catalogs = await validateCatalogs(catalogueDirectory(projectRoot)), reference = catalogs.get("en")!, existing = catalogs.get(tag);
  const directory = external(output, projectRoot);
  await mkdir(directory, {recursive: true});
  await writeFile(join(directory, "language.json"), JSON.stringify(existing?.definition ?? {id: tag, nativeName: tag, direction: "ltr", aliases: [], published: false}, null, 2) + "\n");
  for (const [ns, messages] of Object.entries(reference.messages)) {
    const keys = new Set([...Object.keys(messages), ...Object.keys(existing?.messages[ns] ?? {})]);
    for (const key of Object.keys(messages)) if (key.endsWith("_other") && key.replace(/_other$/, "_one") in messages) {
      for (const category of new Intl.PluralRules(tag).resolvedOptions().pluralCategories) keys.add(key.replace(/_other$/, "_" + category));
    }
    const kit = Object.fromEntries([...keys].map(key => [key, {source: messages[key] ?? messages[key.replace(/_(zero|one|two|few|many|other)$/, "_other")], translation: existing?.messages[ns]?.[key] ?? ""}]));
    await writeFile(join(directory, ns + ".json"), JSON.stringify(kit, null, 2) + "\n");
  }
  return directory;
}
export async function importKit(tag: string, input: string, projectRoot = root) {
  locale(tag);
  if (tag === "en") throw Error("Edit the English source catalogue directly");
  const catalogs = await validateCatalogs(catalogueDirectory(projectRoot)), reference = catalogs.get("en")!;
  const directory = external(input, projectRoot);
  const definition: Definition = {...JSON.parse(await readFile(join(directory, "language.json"), "utf8")), id: tag};
  const errors = definitionErrors(definition), next: Record<string, Messages> = {};
  for (const [other, {definition: known}] of catalogs) if (other !== tag && !errors.length) {
    const tags = [other, ...known.aliases].map(value => Intl.getCanonicalLocales(value)[0]);
    for (const alias of [tag, ...definition.aliases]) if (tags.includes(Intl.getCanonicalLocales(alias)[0])) errors.push(`Ambiguous locale alias: ${alias}`);
  }
  if (errors.length) throw Error(errors.join("\n"));
  for (const file of await readdir(directory)) if (file.endsWith(".json") && file !== "language.json" && !(file.slice(0, -5) in reference.messages)) errors.push(`Unknown namespace: ${file}`);
  for (const [ns, base] of Object.entries(reference.messages)) {
    const kit = JSON.parse(await readFile(join(directory, ns + ".json"), "utf8")), messages: Messages = {};
    for (const [key, entry] of Object.entries(kit)) {
      const record = entry as {source?: unknown; translation?: unknown} | null;
      const value = typeof entry === "string" ? entry : record?.translation;
      if (value === "" || value === undefined) continue;
      if (typeof value !== "string") {errors.push(`${ns}/${key}: translation must be a string`); continue;}
      const source = base[key] ?? base[key.replace(/_(zero|one|two|few|many|other)$/, "_other")];
      if (typeof entry !== "string" && record?.source !== source) errors.push(`${ns}/${key}: source changed; export a fresh kit`);
      messages[key] = value;
    }
    errors.push(...checkMessages(base, messages, tag + "/" + ns, !definition.published, tag)); next[ns] = messages;
  }
  if (errors.length) throw Error(errors.join("\n"));
  // Do not write anything until the complete kit passes validation.
  const destination = join(catalogueDirectory(projectRoot), tag);
  await mkdir(destination, {recursive: true});
  await writeFile(join(destination, "language.json"), JSON.stringify(definition, null, 2) + "\n");
  for (const [ns, messages] of Object.entries(next)) await writeFile(join(destination, ns + ".json"), JSON.stringify(messages, null, 2) + "\n");
  return destination;
}

import {test, expect, afterEach} from "bun:test";
import {mkdtemp, mkdir, writeFile, readFile, rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {checkMessages, definitionErrors, generate, placeholders, validateCatalogs} from "./catalogs";
import {exportKit, importKit} from "./kits";
const folders: string[] = [];
afterEach(async () => {for (const folder of folders.splice(0)) await rm(folder, {recursive: true, force: true});});
async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "peligames-i18n-")); folders.push(root);
  const folder = join(root, "src/i18n/locales/en"); await mkdir(folder, {recursive: true});
  await writeFile(join(folder, "language.json"), JSON.stringify({nativeName: "English", direction: "ltr", aliases: [], published: true}));
  await writeFile(join(folder, "common.json"), JSON.stringify({greet: "Hello {{name}}", count_one: "{{count}} item", count_other: "{{count}} items"}));
  return root;
}
test("current catalogues are complete and preserve variables", async () => {expect((await validateCatalogs()).size).toBeGreaterThanOrEqual(2);});
test("missing keys, misspelled variables and arbitrary plural-looking keys fail", () => {
  const base = {greet: "Hello {{name}}", count_one: "{{count}} item", count_other: "{{count}} items"};
  expect(placeholders("{{count}} / {name}")).toEqual(["count", "name"]);
  expect(checkMessages(base, {greet: "Hola {{user}}"}, "es").join(" ")).toContain("variables differ");
  expect(checkMessages(base, {invented_one: "X"}, "es", true).join(" ")).toContain("unknown key");
  expect(checkMessages(base, {greet: "مرحبا {{name}}", count_one: "{{count}}", count_other: "{{count}}"}, "ar", false, "ar").join(" ")).toContain("count_two");
  expect(definitionErrors({id: "../invalid", nativeName: "x", direction: "rtl", aliases: [], published: true})).not.toBeEmpty();
});
test("export/import supports a third language with extra plural categories and generated lazy loader", async () => {
  const root = await fixture(), kit = join(root, "kit"); await exportKit("ar", kit, root);
  const path = join(kit, "common.json"), messages = JSON.parse(await readFile(path, "utf8"));
  expect(messages.count_few.source).toBe("{{count}} items");
  for (const entry of Object.values(messages) as {source: string; translation: string}[]) entry.translation = entry.source;
  messages.greet.translation = "مرحبا {{name}}";
  await writeFile(path, JSON.stringify(messages));
  await writeFile(join(kit, "language.json"), JSON.stringify({nativeName: "العربية", direction: "rtl", aliases: ["ar-EG"], published: true}));
  await importKit("ar", kit, root); expect(await generate(root)).toEqual({languages: 2, namespaces: 1, messages: 3});
  const registry = await readFile(join(root, "src/i18n/catalog.generated.ts"), "utf8");
  expect(registry).toContain('"direction": "rtl"'); expect(registry).toContain('() => import("./localePacks/ar.generated")');
});
test("incomplete kits remain drafts; invalid imports leave the previous catalogue untouched", async () => {
  const root = await fixture(), kit = join(root, "kit"); await exportKit("es", kit, root);
  await importKit("es", kit, root);
  const target = join(root, "src/i18n/locales/es/common.json"), before = await readFile(target, "utf8");
  const path = join(kit, "common.json"), messages = JSON.parse(await readFile(path, "utf8")); messages.greet.translation = "Hola {{wrong}}";
  await writeFile(path, JSON.stringify(messages)); await expect(importKit("es", kit, root)).rejects.toThrow("variables differ");
  expect(await readFile(target, "utf8")).toBe(before);
  await expect(importKit("../escape", kit, root)).rejects.toThrow();
});

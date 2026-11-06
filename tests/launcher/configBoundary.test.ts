import {expect, test} from "bun:test";
import {readdir, readFile} from "node:fs/promises";
import {join, relative} from "node:path";
import ts from "typescript";

test("production UI sends app configuration persistence through its shared adapter", async () => {
  const root = join(import.meta.dir, "../../src"), violations: string[] = [];
  async function walk(directory: string) {
    for (const entry of await readdir(directory, {withFileTypes: true})) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {if (!["i18n"].includes(entry.name)) await walk(path); continue;}
      if (!/\.tsx?$/.test(entry.name) || entry.name.startsWith("preview") || entry.name.includes(".test.")) continue;
      if (relative(root, path) === "modules/appConfig/appConfig.ts") continue;
      const source = ts.createSourceFile(path, await readFile(path, "utf8"), ts.ScriptTarget.Latest, true);
      function inspect(node: ts.Node) {
        if (ts.isCallExpression(node) && node.arguments[0] && ts.isStringLiteral(node.arguments[0]) && ["load_app_config", "save_app_config"].includes(node.arguments[0].text)) violations.push(relative(root, path));
        ts.forEachChild(node, inspect);
      }
      inspect(source);
    }
  }
  await walk(root); expect(violations).toEqual([]);
});

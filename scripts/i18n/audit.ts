import ts from "typescript";
import {readdir, readFile} from "node:fs/promises";
import {join, relative} from "node:path";
import {root} from "./catalogs";
/** Report legacy visible literals without mistaking paths, log payloads or technical IDs for translations. */
export async function auditSources(directory = join(root, "src")) {
  const literals: {file: string; line: number; text: string}[] = [], binary: {file: string; line: number}[] = [];
  async function walk(path: string) {
    for (const entry of await readdir(path, {withFileTypes: true})) {
      if (entry.isDirectory()) {if (entry.name !== "i18n") await walk(join(path, entry.name)); continue;}
      if (!/\.tsx?$/.test(entry.name) || entry.name.includes(".test.")) continue;
      const filename = join(path, entry.name), source = ts.createSourceFile(filename, await readFile(filename, "utf8"), ts.ScriptTarget.Latest, true);
      function inspect(node: ts.Node) {
        const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
        if (ts.isConditionalExpression(node) && /^(pt|en|!pt|!en|(?:useI18n\(\)\.)?language\s*[!=]==?\s*["'](?:pt|en)["'])$/.test(node.condition.getText(source))) binary.push({file: relative(root, filename), line});
        const text = ts.isJsxText(node) ? node.text.trim() : ts.isJsxAttribute(node) && /^(title|alt|placeholder|aria-label)$/.test(node.name.getText(source)) && node.initializer && ts.isStringLiteral(node.initializer) ? node.initializer.text : "";
        if (/[A-Za-zÀ-ž]{3}/.test(text)) literals.push({file: relative(root, filename), line, text: text.replace(/\s+/g, " ").slice(0, 120)});
        ts.forEachChild(node, inspect);
      }
      inspect(source);
    }
  }
  await walk(directory);
  return {binary, literals};
}

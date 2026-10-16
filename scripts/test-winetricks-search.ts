// Run with: bun scripts/test-winetricks-search.ts
import assert from "node:assert/strict";
import { indexWineCatalog, searchWineCatalog, wineCatalogQuery, WINE_RESULTS_PAGE } from "../src/services/winetricksCatalog";
const catalog = indexWineCatalog([
  { id: "vcrun2022", title: "Visual C++ 2022", category: "dlls", installed: false },
  { id: "corefonts", title: "Fontes básicas", category: "fonts", installed: true },
  ...Array.from({ length: 1000 }, (_, i) => ({ id: `component_${i}`, title: `Component ${i}`, category: "dlls", installed: i === 0 })),
]);
for (const filter of ["all", "dlls", "fonts"] as const) assert.equal(searchWineCatalog(catalog, "", filter).length, 0);
assert.equal(searchWineCatalog(catalog, "", "installed").length, 2);
assert.deepEqual(searchWineCatalog(catalog, wineCatalogQuery("  VCRUN2022  "), "all").map(p => p.id), ["vcrun2022"]);
assert.deepEqual(searchWineCatalog(catalog, wineCatalogQuery("BASICAS"), "fonts").map(p => p.id), ["corefonts"]);
assert.equal(searchWineCatalog(catalog, "font", "dlls").length, 0);
assert.equal(searchWineCatalog(catalog, "component", "installed").length, 1);
assert.equal(searchWineCatalog(catalog, "missing", "all").length, 0);
const broad = searchWineCatalog(catalog, "component", "all");
assert.equal(broad.length, 1000);
assert.equal(broad.slice(0, WINE_RESULTS_PAGE).length, 50);
assert.equal(broad.slice(0, WINE_RESULTS_PAGE * 2).length, 100);
assert.equal(catalog.length, 1002, "search leaves the source catalog intact");
console.log("Passed: empty searches stay empty, installed view, IDs/descriptions/accents, category filters, missing results and 50-item pages across 1002 components.");

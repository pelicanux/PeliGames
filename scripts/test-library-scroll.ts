// Run with: bun scripts/test-library-scroll.ts
// Regression checks for scroll work, dynamic panels and batched cover-cache reads.
import assert from "node:assert/strict";
import { mock } from "bun:test";

let cleanup: (() => void) | undefined;
mock.module("react", () => ({ useEffect: (effect: () => () => void) => { cleanup = effect(); } }));
let discoveries = 0, reads = 0, nextFrame = 0;
const frames = new Map<number, () => void>();
const listeners = new Map<string, (event: any) => void>();
let changed: (records: any[]) => void;
class Element {
  children: Element[] = [];
  parentElement: Element | null = null;
  isConnected = true;
  scrollHeight = 0; clientHeight = 600; scrollTop = 0;
  dataset: Record<string, string> = {};
  style: Record<string, string> = {};
  hidden = false; className = ""; overflowY = "visible";
  classes = new Set<string>();
  classList = { add: (v: string) => this.classes.add(v), remove: (v: string) => this.classes.delete(v), contains: (v: string) => this.classes.has(v), toggle: (v: string, on: boolean) => on ? this.classes.add(v) : this.classes.delete(v) };
  append(...nodes: Element[]) { for (const node of nodes) { node.parentElement = this; this.children.push(node); } }
  contains(node: Element): boolean { return this === node || this.children.some(child => child.contains(node)); }
  setAttribute() {} addEventListener() {} removeEventListener() {}
  getBoundingClientRect() { reads++; return { top: 50, bottom: 650, right: 800, left: 0, width: 800 }; }
  getClientRects() { reads++; return [{}]; }
  remove() { this.isConnected = false; if (this.parentElement) this.parentElement.children = this.parentElement.children.filter(child => child !== this); }
  querySelectorAll() { discoveries++; return hosts; }
}
const body = new Element();
const host = new Element(); host.scrollHeight = 6000; host.overflowY = "auto";
body.append(host);
const hosts = [host];
Object.assign(globalThis, {
  document: { body, createElement: () => new Element(), querySelectorAll: () => [], addEventListener: (type: string, fn: any) => listeners.set(type, fn), removeEventListener: (type: string) => listeners.delete(type) },
  window: { innerHeight: 720, innerWidth: 1000, setTimeout, clearTimeout, addEventListener: () => {}, removeEventListener: () => {} },
  getComputedStyle: (el: Element) => ({ overflowY: el.overflowY, zIndex: "0" }),
  requestAnimationFrame: (fn: () => void) => { frames.set(++nextFrame, fn); return nextFrame; },
  cancelAnimationFrame: (id: number) => frames.delete(id),
  ResizeObserver: class { observe() {} unobserve() {} disconnect() {} },
  MutationObserver: class { constructor(fn: typeof changed) { changed = fn; } observe() {} disconnect() {} },
});
const flush = () => { const pending = [...frames.values()]; frames.clear(); pending.forEach(fn => fn()); };
const { FloatingScrollbars } = await import("../src/components/FloatingScrollbars");
FloatingScrollbars();
assert.equal(discoveries, 1);
const layer = body.children[1];
const thumb = layer.children[0].children[0];
const original = thumb.style.transform;
for (let i = 0; i < 120; i++) { host.scrollTop = i * 10; listeners.get("scroll")!({ target: host }); flush(); }
assert.equal(discoveries, 1, "scroll must never rescan every element of the library");
assert.notEqual(thumb.style.transform, original, "the existing scrollbar must still follow scrolling");
const readsBeforePointer = reads;
for (let i = 0; i < 120; i++) listeners.get("pointermove")!({ clientX: 790, clientY: 100 });
assert.equal(reads, readsBeforePointer, "pointer hover must use cached track bounds, not force layout");
const panel = new Element(); panel.scrollHeight = 2000; panel.overflowY = "auto";
body.append(panel); hosts.push(panel);
changed!([{ target: body }]); flush();
assert.equal(layer.children.length, 2, "new scrolling panels still get a thumb");
panel.isConnected = false;
changed!([{ target: body }]); flush();
assert.equal(layer.children.length, 1, "removed panels release their scrollbars");
cleanup?.();
assert.equal(listeners.size, 0);

const storage = new Map<string, string>();
let automaticReads = 0;
Object.assign(globalThis, { localStorage: { getItem: (key: string) => { if (key === "automatic_cover_fallbacks_v1") automaticReads++; return storage.get(key) ?? null; } } });
storage.set("automatic_cover_fallbacks_v1", JSON.stringify({ "Steam:42": "automatic.jpg" }));
storage.set("custom_covers", JSON.stringify({ "own:Steam:/games/0": "custom.jpg" }));
const { applyCustomCovers } = await import("../src/services/gameLibraryCache");
const games = Array.from({ length: 150 }, (_, index) => ({ name: `Game ${index}`, launcher: "Steam", app_id: "42", path: `/games/${index}`, library_view: "own" as const }));
const decorated = applyCustomCovers(games);
assert.equal(automaticReads, 1, "read and parse the automatic artwork cache once for the whole library");
assert.equal(decorated[0].cover_url, "custom.jpg");
assert.equal(decorated[149].cover_url, "automatic.jpg");
storage.set("automatic_cover_fallbacks_v1", JSON.stringify({ "Steam:42": "updated.jpg" }));
assert.equal(applyCustomCovers(games)[149].cover_url, "updated.jpg", "a later refresh must see cache changes");
console.log("Passed: 120 scroll frames without rediscovery; pointer hover without layout reads; dynamic panels; 150 cover-cache lookups in one read; artwork overrides and refresh.");

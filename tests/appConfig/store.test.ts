import {expect, test} from "bun:test";
import {createAppConfigStore} from "../../src/modules/appConfig/store";
import type {AppConfig} from "../../src/types/appConfig";
const base: AppConfig = {backend: "AMDNR", dll_version: "model.bin", shortcut_key: "Insert", game_shortcut_keys: {existing: "Home"}, custom_game_paths: {existing: "/games/existing"}};
const clone = <T,>(value: T): T => structuredClone(value);
function deferred() {let resolve!: () => void; const promise = new Promise<void>(done => {resolve = done;}); return {promise, resolve};}
const flush = async () => {for (let i = 0; i < 12; i++) await Promise.resolve();};
function fixture() {
  let saved: AppConfig | null = clone(base), fail = false, reads = 0, writes = 0, active = 0, maximum = 0;
  let barrier: Promise<void> | undefined;
  const store = createAppConfigStore({load: async () => {reads++; return clone(saved);}, save: async config => {
    writes++; active++; maximum = Math.max(maximum, active);
    try {await barrier; if (fail) throw Error("disk unavailable"); saved = clone(config);} finally {active--;}
  }});
  return {store, get saved() {return saved;}, set saved(value) {saved = value;}, get reads() {return reads;}, get writes() {return writes;}, get maximum() {return maximum;}, set fail(value: boolean) {fail = value;}, set barrier(value: Promise<void> | undefined) {barrier = value;}};
}
test("one shared initial read, stable snapshots, and no setup inferred from editor defaults", async () => {
  const f = fixture(); f.saved = null;
  const a = f.store.load(), b = f.store.load(); expect(a).toBe(b);
  await Promise.all([a, b]); expect(f.reads).toBe(1);
  expect(f.store.getSnapshot()).toBe(f.store.getSnapshot());
  expect(f.store.getSnapshot()).toMatchObject({config: null, ready: true, loaded: true, pending: 0});
  expect(f.writes).toBe(0);
});
test("overlapping wizard, preference and per-game changes share one ordered writer", async () => {
  const f = fixture(); await f.store.load();
  const gate = deferred(); f.barrier = gate.promise;
  const first = f.store.update({game_shortcut_keys: {gameA: "F4"}}); await flush();
  const second = f.store.update({steamgriddb_api_key: "test-key"});
  const third = f.store.update({backend: "OptiScaler", dll_version: "other.bin", game_shortcut_keys: {gameB: "F6"}});
  expect(f.store.getSnapshot().draft.game_shortcut_keys).toEqual({existing: "Home", gameA: "F4", gameB: "F6"});
  expect(f.store.getSnapshot().config?.backend).toBe("AMDNR");
  gate.resolve(); await Promise.all([first, second, third]);
  expect(f.maximum).toBe(1);
  expect(f.saved).toMatchObject({backend: "OptiScaler", dll_version: "other.bin", steamgriddb_api_key: "test-key", game_shortcut_keys: {existing: "Home", gameA: "F4", gameB: "F6"}, custom_game_paths: {existing: "/games/existing"}});
  expect(f.store.getSnapshot()).toMatchObject({pending: 0, error: "", config: f.saved, draft: f.saved});
});
test("failed writes retain an explicit draft and can be retried without losing confirmed settings", async () => {
  const f = fixture(); await f.store.load(); f.fail = true;
  await expect(f.store.update({scan_steam_protons: true})).rejects.toThrow("disk unavailable");
  expect(f.store.getSnapshot().config?.scan_steam_protons).toBeUndefined();
  expect(f.store.getSnapshot().draft.scan_steam_protons).toBe(true);
  expect(f.store.getSnapshot().pending).toBe(0);
  f.fail = false; await f.store.retry();
  expect(f.saved?.scan_steam_protons).toBe(true); expect(f.store.getSnapshot().error).toBe("");
});
test("a later successful edit also preserves the previous failed intent", async () => {
  const f = fixture(); await f.store.load(); f.fail = true;
  await f.store.update({game_shortcut_keys: {gameA: "F5"}}).catch(() => {});
  f.fail = false; await f.store.update({preferred_proton_family: "ge-proton"});
  expect(f.saved).toMatchObject({game_shortcut_keys: {existing: "Home", gameA: "F5"}, preferred_proton_family: "ge-proton"});
});
test("backend restore and normalization are read before editing and after persistence", async () => {
  const f = fixture(); await f.store.load();
  f.saved = {...base, custom_game_paths: {restored: "/restored"}, game_shortcut_keys: {restored: "End"}};
  await f.store.update({scan_steam_protons: false});
  expect(f.saved).toMatchObject({custom_game_paths: {restored: "/restored"}, game_shortcut_keys: {restored: "End"}, scan_steam_protons: false});
  f.saved = null; await f.store.refresh(); expect(f.store.getSnapshot().config).toBeNull();
});
test("a slow initial load does not discard an edit requested while loading", async () => {
  const gate = deferred(); let saved = clone(base), reads = 0;
  const store = createAppConfigStore({load: async () => {if (++reads === 1) await gate.promise; return clone(saved);}, save: async value => {saved = clone(value);}});
  const loading = store.load(), change = store.update({shortcut_key: "Home"});
  expect(store.getSnapshot().draft.shortcut_key).toBe("Home");
  gate.resolve(); await Promise.all([loading, change]); expect(saved.shortcut_key).toBe("Home");
});
test("read failure is retryable and does not trigger an unsolicited write", async () => {
  let fail = true, writes = 0;
  const store = createAppConfigStore({load: async () => {if (fail) throw Error("unavailable"); return clone(base);}, save: async () => {writes++;}});
  await expect(store.load()).rejects.toThrow("unavailable");
  expect(store.getSnapshot()).toMatchObject({loaded: true, ready: false, loading: false});
  fail = false; await store.retry(); expect(store.getSnapshot().config).toEqual(base); expect(writes).toBe(0);
});

test("refreshing after an external change preserves failed edits and their retry warning", async () => {
  const f = fixture(); await f.store.load(); f.fail = true;
  await f.store.update({scan_steam_protons: true}).catch(() => {});
  f.saved = {...base, preferred_proton_family: "cachyos-proton"};
  await f.store.refresh();
  expect(f.store.getSnapshot().config?.preferred_proton_family).toBe("cachyos-proton");
  expect(f.store.getSnapshot().draft.scan_steam_protons).toBe(true);
  expect(f.store.getSnapshot().error).toContain("disk unavailable");
  f.fail = false; await f.store.retry();
  expect(f.saved).toMatchObject({scan_steam_protons: true, preferred_proton_family: "cachyos-proton"});
});

test("backend-normalized values become the confirmed source for subsequent edits", async () => {
  let saved = clone(base);
  const store = createAppConfigStore({load: async () => clone(saved), save: async value => {saved = {...clone(value), dll_version: "/managed/model.bin"};}});
  await store.load(); await store.update({dll_version: "imported.bin"});
  expect(store.getSnapshot().config?.dll_version).toBe("/managed/model.bin");
  await store.update({scan_steam_protons: true});
  expect(store.getSnapshot().draft.dll_version).toBe("/managed/model.bin");
});

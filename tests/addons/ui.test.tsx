import {expect, spyOn, test} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import type {AddonHost, AddonInstance, AddonModule} from "../../addons/_infra/ui-sdk";
import type {AddonBridge} from "../../src/modules/addons/bridge";
import type {AddonRoute, SelectionSnapshot, UiBundle} from "../../src/modules/addons/contracts";
import {ResourceScope} from "../../src/modules/addons/lifecycle";
import {AddonSession} from "../../src/modules/addons/session";
import {ServiceLeases} from "../../src/modules/addons/serviceLeases";
import {addonDesign} from "../../src/modules/addons/sharedDesign";
import {AddonView} from "../../src/modules/addons/AddonView";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {importAddonModule} from "../../src/modules/addons/moduleImport";
await loadLanguage("pt-BR");

function deferred<T>() {let resolve!: (value: T) => void; let reject!: (error: unknown) => void; const promise = new Promise<T>((yes, no) => {resolve = yes; reject = no}); return {promise, resolve, reject}}
const flush = async () => {for (let i = 0; i < 30; i++) await Promise.resolve()};
const route: AddonRoute = {id: "example", target: "settings", route: "preferences"};
const snapshot: SelectionSnapshot = {example: {version: "1.0.0", digest: "a".repeat(64)}, shared: {version: "1.0.0", digest: "b".repeat(64)}};
const descriptor = (): UiBundle => ({...route, version: "1.0.0", host_api: 1, source: "export function createAddon(){}", source_sha256: "c".repeat(64), selection: snapshot});
class Bridge implements AddonBridge {
  observers = new Set<(value: SelectionSnapshot) => void>();
  loads = 0; stops: {id: string; selection: SelectionSnapshot}[] = [];
  requests: {id: string; selection: SelectionSnapshot; method: string; params: unknown}[] = [];
  loading: () => Promise<UiBundle> = async () => descriptor();
  stopping: () => Promise<void> = async () => {};
  request: () => Promise<unknown> = async () => ({ok: true});
  async observe(callback: (value: SelectionSnapshot) => void) {this.observers.add(callback); return () => {this.observers.delete(callback)}}
  async load() {this.loads++; return this.loading()}
  async call<T>(id: string, selection: SelectionSnapshot, method: string, params: unknown) {this.requests.push({id, selection, method, params}); return await this.request() as T}
  async stop(id: string, selection: SelectionSnapshot) {this.stops.push({id, selection}); await this.stopping()}
  emit(value: SelectionSnapshot) {for (const callback of [...this.observers]) callback(value)}
}
function fixture(bridge = new Bridge(), leases = new ServiceLeases(bridge),design=addonDesign) {
  let host!: AddonHost;
  let disposed = 0, imported = 0;
  const busy: boolean[] = [], invalidations: string[] = [];
  let instance: AddonInstance = {apiVersion: 1, id: "example", version: "1.0.0", views: {preferences: () => "view"}, dispose() {disposed++}};
  let create: (host: AddonHost) => AddonInstance | Promise<AddonInstance> = () => instance;
  const session = new AddonSession(bridge, leases, async () => {imported++; return {createAddon: received => {host = received; return create(received)}}}, design,
    reason => invalidations.push(reason), value => busy.push(value), 100);
  return {bridge, leases, session, busy, invalidations, get host() {return host}, get disposed() {return disposed}, get imported() {return imported},
    replaceInstance(value: AddonInstance) {instance = value}, replaceFactory(value: typeof create) {create = value}};
}
test("view loads lazily, shares real React/design, pins calls and disposes resources once", async () => {
  const f = fixture(); expect(f.imported).toBe(0); expect(f.bridge.requests.length).toBe(0);
  await f.session.open(route, "pt-BR");
  expect(f.host.React).toBe(addonDesign.React); expect(f.host.ui).toBe(addonDesign.ui);
  expect(f.host.language).toBe("pt-BR"); expect(f.bridge.requests.length).toBe(0);
  await f.host.service.call("save", {value: true});
  expect(f.bridge.requests[0]).toEqual({id: "example", selection: snapshot, method: "save", params: {value: true}});
  expect(f.busy).toEqual([true, false]);
  const target = new EventTarget(); let events = 0;
  f.host.lifecycle.listen(target, "ping", () => events++);
  target.dispatchEvent(new Event("ping")); expect(events).toBe(1);
  await f.session.close(); await f.session.close();
  target.dispatchEvent(new Event("ping")); expect(events).toBe(1);
  expect(f.disposed).toBe(1); expect(f.bridge.observers.size).toBe(0); expect(f.bridge.stops).toHaveLength(1);
  await expect(f.host.service.call("save", {})).rejects.toHaveProperty("name", "AbortError");
  expect(() => f.host.lifecycle.timeout(() => {}, 1)).toThrow();
});
test("closing before native source arrives never imports or creates an instance", async () => {
  const f = fixture(); const loading = deferred<UiBundle>(); f.bridge.loading = () => loading.promise;
  const open = f.session.open(route, "en").catch(error => error);
  await flush(); await f.session.close(); loading.resolve(descriptor()); await open; await flush();
  expect(f.imported).toBe(0); expect(f.disposed).toBe(0); expect(f.bridge.observers.size).toBe(0);
});
test("opening the same session twice cannot overwrite its resource ownership", async () => {
  const f = fixture(); await f.session.open(route, "en");
  await expect(f.session.open(route, "en")).rejects.toThrow("once");
  expect(f.bridge.observers.size).toBe(1); expect(f.imported).toBe(1);
  await f.session.close(); expect(f.disposed).toBe(1); expect(f.bridge.observers.size).toBe(0);
});
test("a factory resolving after disposal is cleaned and cannot revive its capabilities", async () => {
  const f = fixture(); const creating = deferred<AddonInstance>(); f.replaceFactory(() => creating.promise);
  const open = f.session.open(route, "en").catch(error => error); await flush();
  expect(f.imported).toBe(1);
  await f.session.close(); let disposed = 0;
  creating.resolve({apiVersion: 1, id: "example", version: "1.0.0", views: {preferences: () => null}, dispose() {disposed++}});
  await open; await flush();
  expect(disposed).toBe(1); expect(f.bridge.stops).toHaveLength(1);
  expect(() => f.host.setBusy(true)).toThrow();
});
test("initialization timeouts release subscriptions and worker leases", async () => {
  const f = fixture(); const creating = deferred<AddonInstance>(); f.replaceFactory(() => creating.promise);
  await expect(f.session.open(route, "en")).rejects.toThrow("timed out");
  expect(f.bridge.observers.size).toBe(0); expect(f.bridge.stops).toHaveLength(1); expect(f.host.lifecycle.signal.aborted).toBe(true);
});
test("identity mismatch or an undeclared view fails closed and disposes the instance", async () => {
  for (const mismatch of ["identity", "route"]) {
    const f = fixture(); let disposed = 0;
    f.replaceInstance({apiVersion: 1, id: mismatch === "identity" ? "wrong" : "example", version: "1.0.0", views: mismatch === "route" ? {} : {preferences: () => null}, dispose() {disposed++}});
    await expect(f.session.open(route, "en")).rejects.toThrow();
    expect(disposed).toBe(1); expect(f.bridge.observers.size).toBe(0); expect(f.bridge.stops).toHaveLength(1);
  }
});
test("a dependency change revokes an open view and is also caught during loading", async () => {
  const changed = {...snapshot, shared: {version: "2.0.0", digest: "d".repeat(64)}};
  const ready = fixture(); await ready.session.open(route, "en"); ready.bridge.emit(changed); await flush();
  expect(ready.invalidations).toEqual(["changed"]); expect(ready.host.lifecycle.signal.aborted).toBe(true); expect(ready.disposed).toBe(1);
  const loading = fixture(); const native = deferred<UiBundle>(); loading.bridge.loading = () => native.promise;
  const open = loading.session.open(route, "en").catch(error => error); await flush(); loading.bridge.emit(changed); native.resolve(descriptor());
  expect((await open).name).toBe("AbortError"); expect(loading.imported).toBe(0); expect(loading.bridge.observers.size).toBe(0);
});
test("the last view releases a shared worker and reopening waits for previous teardown", async () => {
  const bridge = new Bridge(); const leases = new ServiceLeases(bridge);
  const one = fixture(bridge, leases), two = fixture(bridge, leases);
  await one.session.open(route, "en"); await two.session.open(route, "en");
  await one.session.close(); expect(bridge.stops).toHaveLength(0);
  const stop = deferred<void>(); bridge.stopping = () => stop.promise;
  const closing = two.session.close(); await flush(); expect(bridge.stops).toHaveLength(1);
  const three = fixture(bridge, leases); let opened = false;
  const opening = three.session.open(route, "en").then(() => {opened = true}); await flush(); expect(opened).toBe(false);
  stop.resolve(); await closing; await opening;
  expect(opened).toBe(true); await three.session.close(); expect(bridge.stops).toHaveLength(2);
});
test("a pending reply after view disposal cannot return stale data or leave busy set", async () => {
  const f = fixture(); await f.session.open(route, "en"); const work = deferred<unknown>(); f.bridge.request = () => work.promise;
  const call = f.host.service.call("save", {}).catch(error => error); expect(f.busy.at(-1)).toBe(true);
  await f.session.close(); work.resolve({stale: true});
  expect((await call).name).toBe("AbortError"); expect(f.busy.at(-1)).toBe(false);
});
test("tracked timers/listeners and thrown callbacks are stopped without leaking", async () => {
  let calls = 0, failures = 0, cleanups = 0;
  const scope = new ResourceScope(() => failures++);
  scope.onDispose(() => {cleanups++});
  scope.interval(() => {calls++; throw Error("fixture error")}, 1);
  await new Promise(resolve => setTimeout(resolve, 35));
  expect(failures).toBe(1); expect(scope.signal.aborted).toBe(true); expect(cleanups).toBe(1);
  await scope.close(); await new Promise(resolve => setTimeout(resolve, 25)); expect(calls).toBe(1);
});
test("the bundled template uses the host controls and emits no second React import", async () => {
  const path = new URL("../../addons/_infra/template/ui/src/index.tsx", import.meta.url);
  const source = await Bun.file(path).text();
  const transpiler = new Bun.Transpiler({loader: "tsx", tsconfig: JSON.stringify({compilerOptions: {jsx: "react", jsxFactory: "React.createElement"}})});
  const compiled = transpiler.transformSync(source);
  expect(compiled).not.toContain('from "react'); expect(compiled).not.toContain('jsx-runtime');
  const f = fixture(); f.bridge.loading = async () => ({...descriptor(), source: compiled});
  for (const [language, label] of [["pt-BR", "Testar serviço"], ["en", "Test service"]]) {
    const session = new AddonSession(f.bridge, f.leases, importAddonModule, addonDesign, () => {}, () => {});
    const {View} = await session.open(route, language);
    const html = renderToStaticMarkup(<View context={{target: "settings", route: "preferences"}} disabled={false}/>);
    expect(html).toContain('role="switch"'); expect(html).toContain("mod-option-switch--rail"); expect(html).toContain(label);
    await session.close();
  }
});
test("blob URLs are revoked after successful import and syntax failures", async () => {
  const revoke = spyOn(URL, "revokeObjectURL");
  try {
    const module = await importAddonModule({...descriptor(), source: "export function createAddon() {}"});
    expect(typeof module.createAddon).toBe("function");
    await expect(importAddonModule({...descriptor(), source: "export ???"})).rejects.toThrow();
    expect(revoke).toHaveBeenCalledTimes(2);
  } finally {revoke.mockRestore()}
});
test("the host view keeps existing loading copy and both language catalogs", () => {
  const html = renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><AddonView route={route}/></I18nProvider>);
  expect(html).toContain("Abrindo complemento"); expect(html).toContain('role="status"');
});

test("file pickers and machine context are available only while their add-on session is open",async()=>{
 const calls:string[]=[];const design={...addonDesign,files:{async openDirectory(){},async chooseFile(label:string,extensions:string[]){calls.push(label+extensions.join(','));return '/fixture/Lossless.dll'},async chooseDirectory(){calls.push('directory');return '/fixture'},async machine(){calls.push('machine');return {gpus:[],steam_libraries:['/fixture/Steam']}}}};
 const f=fixture(undefined,undefined,design);await f.session.open(route,'en');
 expect(await f.host.files.chooseFile!('DLL',['dll'])).toBe('/fixture/Lossless.dll');expect(await f.host.files.chooseDirectory!('folder')).toBe('/fixture');expect((await f.host.files.machine!()).steam_libraries).toEqual(['/fixture/Steam']);
 await f.session.close();await expect(f.host.files.chooseFile!('DLL',['dll'])).rejects.toHaveProperty('name','AbortError');await expect(f.host.files.machine!()).rejects.toHaveProperty('name','AbortError');expect(calls).toEqual(['DLLdll','directory','machine']);
});

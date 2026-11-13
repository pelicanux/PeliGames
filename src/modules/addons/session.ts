import type {AddonHost, AddonInstance, AddonModule, AddonViewProps, AddonGameActions} from "../../../addons/_infra/ui-sdk";
import type {AddonBridge} from "./bridge";
import {closedError, selectionCurrent, type AddonRoute, type SelectionSnapshot, type UiBundle} from "./contracts";
import {ResourceScope, settleWithin} from "./lifecycle";
import type {ServiceLease, ServiceLeases} from "./serviceLeases";
import {createAddonI18n} from "./i18n";
import {createAddonStyles} from "./styles";

type HostDesign = Pick<AddonHost, "React" | "ui" | "files">;
export interface OpenedAddon {View: (props: AddonViewProps) => import("react").ReactNode; instance: AddonInstance}
function guarded<T>(work: Promise<T>, signal: AbortSignal, milliseconds: number, late?: (value: T) => void): Promise<T> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (action: () => void) => {
      if (settled) return; settled = true; clearTimeout(timer); signal.removeEventListener("abort", abort); action();
    };
    const abort = () => finish(() => reject(closedError()));
    const timer = setTimeout(() => finish(() => reject(new Error("Add-on initialization timed out"))), milliseconds);
    signal.addEventListener("abort", abort, {once: true});
    if (signal.aborted) abort();
    work.then(value => {if (settled) {try {late?.(value)} catch {/* Late resources cannot reopen a disposed session. */}} else finish(() => resolve(value))}, error => finish(() => reject(error)));
  });
}
function validBundle(bundle: UiBundle, request: AddonRoute): boolean {
  return bundle.id === request.id && bundle.target === request.target && bundle.route === request.route && bundle.host_api === 1
    && typeof bundle.source === "string" && bundle.source.length > 0 && bundle.source.length <= 2 * 1024 * 1024
    && /^[a-f0-9]{64}$/.test(bundle.source_sha256) && bundle.selection[bundle.id]?.version === bundle.version;
}
/** A view owns its timers/subscriptions, module instance and shared service lease. */
export class AddonSession {
  readonly scope: ResourceScope;
  private unsubscribe?: () => void;
  private lease?: ServiceLease;
  private instance?: AddonInstance;
  private disposed = new WeakSet<object>();
  private latest?: SelectionSnapshot;
  private selection?: SelectionSnapshot;
  private closing?: Promise<void>;
  private inflight = 0;
  private customBusy = false;
  private started = false;
  constructor(private bridge: AddonBridge, private leases: ServiceLeases, private importer: (bundle: UiBundle) => Promise<AddonModule>,
    private design: HostDesign, private onInvalidated: (reason: "changed" | "failed") => void, private onBusy: (busy: boolean) => void, private deadline = 5000, private game?: AddonGameActions) {
    this.scope = new ResourceScope(() => {this.onInvalidated("failed"); void this.close()});
  }
  async open(request: AddonRoute, language: string): Promise<OpenedAddon> {
    if (this.started) throw new Error("An add-on session can only be opened once");
    this.started = true;
    try {
      this.scope.assertOpen();
      const unsubscribe = await guarded(this.bridge.observe(snapshot => {
        this.latest = snapshot;
        if (this.selection && !selectionCurrent(this.selection, snapshot)) {this.onInvalidated("changed"); void this.close()}
      }), this.scope.signal, this.deadline, release => release());
      if (this.scope.signal.aborted) {unsubscribe(); throw closedError()}
      this.unsubscribe = unsubscribe;
      const bundle = await guarded(this.bridge.load(request), this.scope.signal, this.deadline);
      if (!validBundle(bundle, request)) throw new Error("Invalid add-on UI descriptor");
      this.selection = bundle.selection;
      if (this.latest && !selectionCurrent(bundle.selection, this.latest)) throw closedError();
      this.scope.assertOpen();
      const module = await guarded(this.importer(bundle), this.scope.signal, this.deadline);
      this.scope.assertOpen();
      if (typeof module.createAddon !== "function") throw new Error("Missing add-on factory");
      const lease = await guarded(this.leases.acquire(bundle.id, bundle.selection, this.scope.signal), this.scope.signal, this.deadline, lease => {void lease.release()});
      if (this.scope.signal.aborted) {void lease.release(); throw closedError()}
      this.lease = lease;
      const busy = () => {if (!this.scope.signal.aborted) this.onBusy(this.customBusy || this.inflight > 0)};
      const host: AddonHost = Object.freeze({apiVersion: 1, addon: Object.freeze({id: bundle.id, version: bundle.version}), ...this.design, lifecycle: this.scope, language, i18n: createAddonI18n(language, this.scope),
        game: this.game && Object.freeze({
          launch: this.game.launch && Object.freeze({
            read: async (gameId: string) => {this.scope.assertOpen();const result=await this.game!.launch!.read(gameId);this.scope.assertOpen();return result},
            save: async (gameId: string,data:Record<string,unknown>) => {this.scope.assertOpen();await this.game!.launch!.save(gameId,data);this.scope.assertOpen()},
          }),
          synchronizeLaunch: async (...args: Parameters<AddonGameActions["synchronizeLaunch"]>) => {this.scope.assertOpen(); const value = await this.game!.synchronizeLaunch(...args); this.scope.assertOpen(); return value},
          saveMenuShortcut: async (...args: Parameters<AddonGameActions["saveMenuShortcut"]>) => {this.scope.assertOpen(); await this.game!.saveMenuShortcut(...args); this.scope.assertOpen()},
          refresh: async (...args: Parameters<AddonGameActions["refresh"]>) => {this.scope.assertOpen(); await this.game!.refresh(...args); this.scope.assertOpen()},
          chooseExecutableDirectory: async (gameId: string) => {this.scope.assertOpen(); await this.game!.chooseExecutableDirectory(gameId); this.scope.assertOpen()},
          openGroup: (group: string) => {this.scope.assertOpen(); this.game!.openGroup(group)},
        }),
        styles: createAddonStyles(this.scope, bundle.id, typeof document === "undefined" ? undefined : document),
        files: Object.freeze({
          ...(this.design.files.openFile?{openFile:async(path:string)=>{this.scope.assertOpen();await this.design.files.openFile!(path);this.scope.assertOpen()}}:{}),
          openDirectory: async (path: string) => {this.scope.assertOpen(); await this.design.files.openDirectory(path); this.scope.assertOpen()},
          ...(this.design.files.chooseFile?{chooseFile:async(label:string,extensions:string[])=>{this.scope.assertOpen();const value=await this.design.files.chooseFile!(label,extensions);this.scope.assertOpen();return value}}:{}),
          ...(this.design.files.chooseDirectory?{chooseDirectory:async(label:string)=>{this.scope.assertOpen();const value=await this.design.files.chooseDirectory!(label);this.scope.assertOpen();return value}}:{}),
          ...(this.design.files.machine?{machine:async()=>{this.scope.assertOpen();const value=await this.design.files.machine!();this.scope.assertOpen();return value}}:{}),
        }),
        service: Object.freeze({call: async <T,>(method: string, params: unknown) => {
          this.scope.assertOpen(); this.inflight++; busy();
          try {return await lease.call<T>(method, params)} finally {this.inflight--; busy()}
        }}),
        setBusy: (value: boolean) => {this.scope.assertOpen(); this.customBusy = value; busy()},
      });
      const instance = await guarded(Promise.resolve(module.createAddon(host)), this.scope.signal, this.deadline, instance => {void this.dispose(instance)});
      this.instance = instance;
      if (this.scope.signal.aborted) {void this.dispose(instance); throw closedError()}
      if (!instance || typeof instance !== "object" || instance.apiVersion !== bundle.host_api || instance.id !== bundle.id || instance.version !== bundle.version
        || typeof instance.dispose !== "function" || typeof instance.views?.[bundle.route] !== "function") throw new Error("Add-on UI identity or route mismatch");
      this.scope.assertOpen();
      return {View: instance.views[bundle.route], instance};
    } catch (error) {await this.close(); throw error}
  }
  private dispose(instance?: AddonInstance): Promise<void> {
    if (!instance || typeof instance !== "object" || this.disposed.has(instance)) return Promise.resolve();
    this.disposed.add(instance);
    return settleWithin([Promise.resolve().then(() => {if (typeof instance.dispose === "function") return instance.dispose()})], 1000);
  }
  close(): Promise<void> {
    if (this.closing) return this.closing;
    let finish!: () => void;
    this.closing = new Promise<void>(resolve => {finish = resolve});
    const resources = this.scope.close();
    const unsubscribe = this.unsubscribe;
    this.unsubscribe = undefined;
    try {unsubscribe?.()} catch {/* Continue disposing the instance and worker lease. */}
    try {this.onBusy(false)} catch {/* Cleanup does not depend on the outer view. */}
    void Promise.allSettled([resources, this.dispose(this.instance), this.lease?.release()]).then(finish);
    return this.closing;
  }
}

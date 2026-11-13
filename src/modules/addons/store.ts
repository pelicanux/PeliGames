import {useSyncExternalStore} from "react";
import {Channel, invoke} from "@tauri-apps/api/core";
import {addonBridge} from "./bridge";
import type {AddonOverview,LocalAddonPlan} from "./contracts";

export interface AddonProgress {id: string; phase: string; downloaded: number; total: number}
export interface StoreSnapshot {
  overview?: AddonOverview;
  loading: boolean;
  busy: string | null;
  error: string;
  progress?: AddonProgress;
}
export interface StoreBridge {
  read(): Promise<AddonOverview>;
  observe(refresh: () => void): Promise<() => void>;
  command<T>(name: string, args: Record<string, unknown>): Promise<T>;
  install(id: string, progress: (value: AddonProgress) => void): Promise<unknown>;
}

/** One authoritative metadata snapshot; views never mutate the package registry. */
export function createAddonStore(bridge: StoreBridge) {
  let state: StoreSnapshot = {loading: true, busy: null, error: ""};
  let revision = 0, generation = 0, operationId = 0;
  let unsubscribe: (() => void) | undefined;
  const listeners = new Set<() => void>();
  const publish = (patch: Partial<StoreSnapshot>) => {
    state = {...state, ...patch};
    for (const listener of listeners) listener();
  };
  const refresh = async () => {
    const request = ++revision;
    try {
      const overview = await bridge.read();
      if (request === revision) publish({overview, loading: false, error: ""});
    } catch (error) {
      if (request === revision) publish({loading: false, error: String(error)});
    }
  };
  const run = async (id: string, operation: () => Promise<unknown>) => {
    if (state.busy) return;
    const own = ++operationId;
    publish({busy: id, error: "", progress: undefined});
    try {
      await operation();
      await refresh();
      return true;
    } catch (error) {
      // A dependency/pin failure may follow a successful disable. Reload native
      // state without hiding the operation error or pretending to roll it back.
      await refresh();
      publish({error: String(error)});
      return false;
    } finally {
      if (own === operationId) publish({busy: null, progress: undefined});
    }
  };
  return {
    getSnapshot: () => state,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      if (listeners.size === 1) {
        const own = ++generation;
        void bridge.observe(() => void refresh()).then(release => {
          if (own !== generation || !listeners.size) release();
          else {
            unsubscribe = release;
            void refresh(); // Close the initial read/observer registration race.
          }
        }, error => {
          if (own === generation && listeners.size) publish({loading: false, error: String(error)});
        });
        void refresh();
      }
      return () => {
        listeners.delete(listener);
        if (!listeners.size) {
          generation++;
          revision++;
          unsubscribe?.();
          unsubscribe = undefined;
        }
      };
    },
    refresh,
    refreshCatalog: () => run("catalog", () => bridge.command("addon_refresh_catalog", {})),
    inspectLocal: (path:string) => bridge.command<LocalAddonPlan>("addon_local_plan",{path}),
    importLocal: (path:string,sha256:string) => run("local-import",()=>bridge.command("addon_import_local",{path,sha256})),
    install: (id: string) => run(id, async () => {
      await bridge.command("addon_install_plan", {ids: [id]});
      const own = operationId;
      await bridge.install(id, progress => {
        if (own === operationId && state.busy === id) publish({progress});
      });
    }),
    enable: (id: string, enabled: boolean) => run(id, () => bridge.command("addon_set_enabled", {id, enabled})),
    select: (id: string, version: string) => run(id, () => bridge.command("addon_select_version", {id, version})),
    remove: (id: string) => run(id, async () => {
      const addon = state.overview?.state.addons[id];
      if (!addon) return;
      if (addon.enabled) await bridge.command("addon_set_enabled", {id, enabled: false});
      for (const version of Object.keys(addon.versions)) {
        await bridge.command("addon_remove_version", {id, version});
      }
    }),
    complete: () => run("selection", () => bridge.command("addon_complete_selection", {})),
  };
}
export type AddonStoreController = ReturnType<typeof createAddonStore>;
export const addonStore = createAddonStore({
  read: () => invoke<AddonOverview>("addon_store_state"),
  observe: refresh => addonBridge.observe(refresh),
  command: invoke,
  install: (id, progress) => {
    const channel = new Channel<AddonProgress>();
    channel.onmessage = progress;
    return invoke("addon_install", {ids: [id], onProgress: channel});
  },
});
export const useAddonStore = (controller = addonStore) =>
  useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot);

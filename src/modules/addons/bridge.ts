import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import type {AddonRoute, SelectionSnapshot, UiBundle, UiContribution} from "./contracts";

export interface AddonBridge {
  load(route: AddonRoute): Promise<UiBundle>;
  observe(callback: (snapshot: SelectionSnapshot) => void): Promise<() => void>;
  call<T>(id: string, selection: SelectionSnapshot, method: string, params: unknown): Promise<T>;
  stop(id: string, selection: SelectionSnapshot): Promise<void>;
}
export const addonBridge: AddonBridge = {
  load: ({id, target, route}) => invoke("addon_ui_load", {id, target, route}),
  observe: callback => listen<SelectionSnapshot>("addon-store-changed", event => callback(event.payload)),
  call: (id, expectedSelection, method, params) => invoke("addon_service_call", {id, expectedSelection, method, params}),
  stop: (id, expectedSelection) => invoke("addon_service_stop", {id, expectedSelection}),
};
export const readContributions = () => invoke<UiContribution[]>("addon_ui_contributions");

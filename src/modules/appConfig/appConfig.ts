import {invoke} from "@tauri-apps/api/core";
import {createAppConfigStore} from "./store";
import type {AppConfig} from "../../types/appConfig";

export const appConfigStore = createAppConfigStore({
  load: () => invoke<AppConfig | null>("load_app_config"),
  save: config => invoke("save_app_config", {config}),
  committed: (previous, next) => {
    if (previous?.steamgriddb_api_key !== next?.steamgriddb_api_key) window.dispatchEvent(new Event("steamGridSettingsChanged"));
    if (previous?.scan_steam_protons !== next?.scan_steam_protons) window.dispatchEvent(new Event("protonRunnersChanged"));
  },
});

// Pelinstall and the launcher can share storage across separate windows/processes.
// Reconcile when this window becomes active, preserving pending local edits.
if (typeof window !== "undefined") {
  const refresh = () => {void appConfigStore.refresh().catch(() => {});};
  window.addEventListener("focus", refresh);
  import.meta.hot?.dispose(() => window.removeEventListener("focus", refresh));
}

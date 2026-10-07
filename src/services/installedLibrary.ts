import { applyCustomCovers } from "./gameLibraryCache";
import { invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";
export async function listPeliGamesEntries(rescan = false): Promise<GameInfo[]> {
  if (!("__PELI_UI_PREVIEW__" in window)) return invoke("list_peligames_entries", { rescan });
  const response = await fetch("/__preview/peligames-library", { method: rescan ? "POST" : "GET" });
  const body = await response.json();
  if (!response.ok || body.error) throw new Error(body.error || "Falha ao carregar a biblioteca PeliGames.");
  return body.result;
}

export interface EntrySettings { advanced?: import("./advancedSettings").AdvancedSettings; prefix?: string; path: string; name: string; executable: string; proton: string; }
export async function updatePeliGamesSettings(settings: EntrySettings): Promise<GameInfo> {
  if (!("__PELI_UI_PREVIEW__" in window)) return applyCustomCovers([await invoke<GameInfo>("update_peligames_settings", { settings })])[0];
  const response = await fetch("/__preview/peligames-settings", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(settings) });
  const body = await response.json();
  if (!response.ok || body.error) throw new Error(body.error || "Falha ao salvar configurações.");
  return applyCustomCovers([body.result])[0];
}

async function mutateLibrary<T>(command: string, route: string, args: Record<string, unknown>, body: unknown): Promise<T> {
  if (!("__PELI_UI_PREVIEW__" in window)) return invoke<T>(command, args);
  const response = await fetch(route, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  const result = await response.json();
  if (!response.ok || result.error) throw new Error(result.error || "Falha ao atualizar a biblioteca.");
  return result.result;
}
export function addPeliGamesEntry(request: import("./gameInstallation").GameInstallationRequest): Promise<GameInfo> {
  return mutateLibrary("add_peligames_entry", "/__preview/peligames-add", { request }, request);
}
export function uninstallPeliGamesEntry(path: string): Promise<void> {
  return mutateLibrary("uninstall_peligames_entry", "/__preview/peligames-uninstall", { path }, { path });
}

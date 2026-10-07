import { invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";
import type { GameInstallationResult } from "./gameInstallation";
export interface ExecutableMatch { shortcuts: string[] | null; entry: GameInfo; destination: string; installer: string | null; }
export async function findPelinstallMatches(executable: string): Promise<ExecutableMatch[]> {
  if (!("__PELI_UI_PREVIEW__" in window)) return invoke("find_pelinstall_matches", { executable });
  const response = await fetch("/__preview/pelinstall-matches", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ executable }) });
  const body = await response.json();
  if (!response.ok || body.error) throw new Error(body.error || "Não foi possível consultar a biblioteca.");
  return body.result;
}
export function repairPeliGamesEntry(request: { path: string; name: string; proton: string; prefix: string; executable: string }): Promise<GameInstallationResult> {
  return invoke("repair_peligames_entry", { request });
}

export async function getPelinstallIcon(path: string): Promise<number[] | null> {
  if (!("__PELI_UI_PREVIEW__" in window)) return invoke("get_pelinstall_icon", { path });
  const response = await fetch("/__preview/pelinstall-icon", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ executable: path }) });
  const body = await response.json();
  if (!response.ok || body.error) throw new Error(body.error || "Não foi possível ler o ícone.");
  return body.result;
}

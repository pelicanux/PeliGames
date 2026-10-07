import { invoke } from "@tauri-apps/api/core";
export interface GameInstallationRequest { name: string; directory: string; executable: string; proton: string; cover_url?: string; }
export interface GameInstallationResult { prefix: string; log: string; registered_count: number; library_error?: string; }
export async function runGameInstaller(request: GameInstallationRequest): Promise<GameInstallationResult> {
  if (!("__PELI_UI_PREVIEW__" in window)) return invoke("run_game_installer", { request });
  const response = await fetch("/__preview/game-installer", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(request) });
  const result = await response.json();
  if (!response.ok || result.error) throw new Error(result.error || "Falha ao abrir o instalador.");
  return result.result;
}

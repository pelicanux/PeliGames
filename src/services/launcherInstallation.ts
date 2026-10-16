import type { GameInfo } from "../components/GameGrid";
import { invoke } from "@tauri-apps/api/core";
import ubisoft from "../../installers/recipes/ubisoft-connect.json";
import ea from "../../installers/recipes/ea-app.json";
import battle from "../../installers/recipes/battle-net.json";
// In-memory fixtures only: never write the user's native library from preview.
export const previewLauncherEntries: GameInfo[] = [];
export const launcherRecipes = [ubisoft, ea, battle];
export type LauncherRecipe = typeof launcherRecipes[number];
export interface LauncherInstallRequest { id: string; directory: string; proton: string; dependencies: string[]; }
export interface LauncherInstallResult { prefix: string; log: string; registered_count: number; library_error?: string; }
export async function installLauncher(request: LauncherInstallRequest): Promise<LauncherInstallResult> {
  if (import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window) {
    await new Promise(resolve => setTimeout(resolve, 1600));
    const recipe=launcherRecipes.find(item=>item.id===request.id);
    if (!recipe) throw new Error("Launcher desconhecido.");
    const prefix=request.directory;
    const executable=`${prefix}/${recipe.executable.relative_paths[0]}`;
    const entry:GameInfo={name:`${recipe.name} — demonstração`,path:executable,executable,prefix,directory:request.directory,proton:request.proton,launcher:"PeliGames",library_category:"launchers",launcher_id:recipe.id,cover_url:`/launchers/${recipe.id}-cover.svg`,library_view:"own"};
    const index=previewLauncherEntries.findIndex(item=>item.path===entry.path);
    if (index>=0) previewLauncherEntries[index]=entry; else previewLauncherEntries.push(entry);
    return { prefix, log: "", registered_count: 1 };
  }
  return invoke("install_launcher", { request });
}
export function cancelLauncherInstallation() { return invoke("cancel_launcher_installation"); }

export function launcherCoverFallback(game: GameInfo): string | undefined {
  if (game.library_category !== "launchers") return undefined;
  const recipe=launcherRecipes.find(item=>item.id===game.launcher_id || game.name.startsWith(item.name));
  return recipe ? `/launchers/${recipe.id}-cover.svg` : undefined;
}

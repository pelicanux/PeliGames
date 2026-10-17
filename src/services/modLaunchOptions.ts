import { invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";
import { normalizeAdvanced } from "./advancedSettings";
import { updatePeliGamesSettings } from "./installedLibrary";

/** Where a Steam game's mod options went: written into Steam's own launch
 * options, or waiting for Steam to close (it rewrites that file on exit). */
export type SteamModOptions = "written" | "waiting";
export type SyncedGame = GameInfo & { steamModOptions?: SteamModOptions };

/** Puts the launch options a mod needs (the backend's printed line, or none
 * after a removal) where the game reads them, replacing what the mod added
 * before: Steam's own launch options for Steam games (they apply however the
 * game is opened, and an open Steam does not block it), the game's PeliGames
 * launch options otherwise. Returns the saved game, or null when the game has
 * no settings to hold them (then the user copies the line by hand). */
export async function syncModLaunchOptions(game: GameInfo | null | undefined, gameDir: string, line: string | null): Promise<SyncedGame | null> {
  if (!game || "__PELI_UI_PREVIEW__" in window) return null;
  if (!game.external_id && game.launcher !== "PeliGames") return null;
  const current = game.advanced?.launch_options ?? null;
  const steam = game.launcher === "Steam" && game.external_id && game.app_id
    ? await invoke<{ launch_options: string | null; waiting: boolean }>("apply_steam_mod_launch_options", { appId: game.app_id, gameDir, current, line })
    : null;
  const next = steam ? steam.launch_options : await invoke<string | null>("apply_mod_launch_options", { gameDir, current, line });
  const steamModOptions: SteamModOptions | undefined = steam ? (steam.waiting ? "waiting" : "written") : undefined;
  if ((next ?? null) === current) return { ...game, steamModOptions };
  const advanced = { ...normalizeAdvanced(game.advanced), launch_options: next };
  const updated = game.external_id
    ? await invoke<GameInfo>("configure_external_game", { settings: { external_id: game.external_id, advanced } })
    : await updatePeliGamesSettings({ path: game.path, name: game.name, proton: game.proton || "", executable: game.executable || game.path, prefix: game.prefix, advanced });
  window.dispatchEvent(new Event("peligamesLibraryChanged"));
  // Only the settings change: the caller's game keeps its own folder and paths
  // (the DLSS view uses the executable's folder, not the launcher's record).
  return { ...game, advanced: updated.advanced ?? advanced, steamModOptions };
}

import { invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";
import { normalizeAdvanced } from "./advancedSettings";
import { updatePeliGamesSettings } from "./installedLibrary";

/** Puts the launch options a mod needs (the backend's printed line, or none
 * after a removal) into the game's own PeliGames launch options, replacing
 * what the mod added before. Returns the saved game, or null when the game has
 * no PeliGames settings to hold them (then the user copies the line by hand). */
export async function syncModLaunchOptions(game: GameInfo | null | undefined, gameDir: string, line: string | null): Promise<GameInfo | null> {
  if (!game || "__PELI_UI_PREVIEW__" in window) return null;
  if (!game.external_id && game.launcher !== "PeliGames") return null;
  const current = game.advanced?.launch_options ?? null;
  const next = await invoke<string | null>("apply_mod_launch_options", { gameDir, current, line });
  if ((next ?? null) === current) return game;
  const advanced = { ...normalizeAdvanced(game.advanced), launch_options: next };
  const updated = game.external_id
    ? await invoke<GameInfo>("configure_external_game", { settings: { external_id: game.external_id, advanced } })
    : await updatePeliGamesSettings({ path: game.path, name: game.name, proton: game.proton || "", executable: game.executable || game.path, prefix: game.prefix, advanced });
  window.dispatchEvent(new Event("peligamesLibraryChanged"));
  return { ...updated, library_view: game.library_view, ...(game.external_id ? { cover_url: game.cover_url, automatic_cover_url: game.automatic_cover_url } : {}) };
}

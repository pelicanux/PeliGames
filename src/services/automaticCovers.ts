import { invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";

const CACHE_KEY = "automatic_cover_fallbacks_v1";
// Steam IDs survive installation-path changes and are shared across libraries.
export function automaticCoverKey(game: GameInfo): string {
  return game.launcher === "Steam" && game.app_id ? `Steam:${game.app_id}` : `${game.launcher}:${game.path}`;
}
export function readAutomaticCovers(): Record<string, string> {
  try {
    const value = JSON.parse(localStorage.getItem(CACHE_KEY) || "{}");
    return value && typeof value === "object" && !Array.isArray(value) ? value : {};
  } catch { return {}; }
}
export function automaticCoverFor(game: GameInfo, covers = readAutomaticCovers()): string | undefined {
  const value = covers[automaticCoverKey(game)];
  return typeof value === "string" && value ? value : undefined;
}
export async function rememberAutomaticCover(game: GameInfo, remoteUrl: string): Promise<string> {
  let url = remoteUrl;
  // The preview retains its simulated URLs; only the native app downloads a copy.
  if (!(import.meta.env?.DEV && (window as Window & { __PELI_UI_PREVIEW__?: boolean }).__PELI_UI_PREVIEW__)) {
    try { url = await invoke<string>("cache_automatic_game_cover", { url: remoteUrl }); }
    catch (error) { console.warn("Could not cache automatic artwork locally", error); }
  }
  try { localStorage.setItem(CACHE_KEY, JSON.stringify({ ...readAutomaticCovers(), [automaticCoverKey(game)]: url })); }
  catch (error) { console.warn("Could not remember automatic artwork", error); }
  return url;
}
export function forgetAutomaticCover(game: GameInfo) {
  const covers = readAutomaticCovers();
  delete covers[automaticCoverKey(game)];
  try { localStorage.setItem(CACHE_KEY, JSON.stringify(covers)); }
  catch (error) { console.warn("Could not reset automatic artwork", error); }
}

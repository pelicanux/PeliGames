import { STEAMGRIDDB_API_KEY } from "./coverConfig";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { gameViewKey } from "./gameIdentity";
import type { GameInfo } from "../components/GameGrid";
import { openFilePicker } from "./tauriService";

/** Rewrites cover addresses saved before the launcher data moved to
 * `~/.config/PeliGames` (covers used to live in `~/.local/share`). */
export async function migrateCoverPaths() {
  let directories: [string | null, string];
  try { directories = await invoke<[string | null, string]>("cover_directories"); } catch { return; }
  const [legacy, current] = directories;
  if (!legacy || legacy === current) return;
  const pairs = [[legacy, current], [encodeURIComponent(legacy), encodeURIComponent(current)]];
  try {
    for (let index = 0; index < localStorage.length; index++) {
      const key = localStorage.key(index);
      if (!key || !(key === "custom_covers" || key === "automatic_cover_fallbacks_v1" || key.startsWith("game_library_cache_v1"))) continue;
      const value = localStorage.getItem(key) ?? "";
      const updated = pairs.reduce((text, [from, to]) => text.split(from).join(to), value);
      if (updated !== value) localStorage.setItem(key, updated);
    }
  } catch { /* storage unavailable: covers fall back to the automatic artwork */ }
}

export function readCustomCovers(): Record<string, string> {
  try {
    const saved = JSON.parse(localStorage.getItem("custom_covers") || "{}");
    return saved && typeof saved === "object" && !Array.isArray(saved) ? saved : {};
  } catch { return {}; }
}

export function customCoverFor(game: GameInfo, covers = readCustomCovers()): string | undefined {
  const key = gameViewKey(game);
  // An empty scoped value resets a legacy shared cover in this library only.
  return Object.prototype.hasOwnProperty.call(covers, key) ? covers[key] : covers[game.path];
}
export function hasCustomCover(game: GameInfo) { return Boolean(customCoverFor(game)); }

export async function fetchGameCoverByName(name: string) {
  return invoke<string | null>("fetch_steamgriddb_cover_command", { name: name.trim(), apiKey: STEAMGRIDDB_API_KEY });
}

function publish(game: GameInfo) {
  window.dispatchEvent(new CustomEvent("gameCoverChanged", { detail: game }));
}

export async function changeLocalCover(game: GameInfo) {
  const file = await openFilePicker("Images", ["png", "jpg", "jpeg", "webp"]);
  if (!file) return;
  const savedPath = await invoke<string>("import_game_cover", { source: file });
  const url = convertFileSrc(savedPath);
  // Only replace a working cover after the imported image can actually be displayed.
  await new Promise<void>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve();
    image.onerror = () => reject(new Error("Could not load the selected image"));
    image.src = url;
  });
  localStorage.setItem("custom_covers", JSON.stringify({ ...readCustomCovers(), [gameViewKey(game)]: url }));
  publish({ ...game, automatic_cover_url: game.automatic_cover_url !== undefined ? game.automatic_cover_url : game.cover_url ?? null, cover_url: url });
}

export async function resetGameCover(game: GameInfo) {
  let url = game.automatic_cover_url;
  if (!url || url.startsWith("asset:")) {
    url = game.app_id ? `https://steamcdn-a.akamaihd.net/steam/apps/${game.app_id}/library_600x900.jpg`
      : await fetchGameCoverByName(game.name);
  }
  const covers = readCustomCovers();
  covers[gameViewKey(game)] = "";
  localStorage.setItem("custom_covers", JSON.stringify(covers));
  publish({ ...game, automatic_cover_url: url ?? null, cover_url: url ?? undefined });
}

export function reportCoverError(error: unknown) {
  window.dispatchEvent(new CustomEvent("gameCoverError", { detail: String(error) }));
}

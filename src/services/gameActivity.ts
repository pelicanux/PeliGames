import { invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";

/** Last start (unix seconds) and total minutes, from the game's own launcher. */
export interface GameActivity { last_played: number; minutes: number }
export type ActivityMap = Record<string, GameActivity>;

/** Same keys as the backend's game_activity command. */
export function activityKey(game: GameInfo): string | null {
  if (game.launcher === "PeliGames") return `PeliGames:${game.path}`;
  if (!game.app_id) return null;
  if (game.launcher === "Steam") return `Steam:${game.app_id}`;
  if (game.launcher === "Heroic" || game.launcher === "Lutris") return `${game.launcher}:${game.flatpak ? "flatpak" : "native"}:${game.app_id}`;
  return null;
}

export function activityOf(activity: ActivityMap, game: GameInfo): GameActivity | undefined {
  const key = activityKey(game);
  return key ? activity[key] : undefined;
}

export async function loadGameActivity(): Promise<ActivityMap> {
  try { return await invoke<ActivityMap>("game_activity"); } catch { return {}; }
}

/** "46 h", "35 min"; empty below a minute. */
export function formatPlaytime(minutes: number): string {
  if (minutes < 1) return "";
  return minutes < 60 ? `${minutes} min` : `${Math.round(minutes / 60).toLocaleString()} h`;
}

/** Whole days between the last start and today (0 today, 1 yesterday). */
export function daysSince(seconds: number, now = new Date()): number {
  const start = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  return Math.round((start(now) - start(new Date(seconds * 1000))) / 86_400_000);
}

import type { GameInfo } from "../components/GameGrid";

/** The same program can have separate launcher and mod-manager presentations. */
export function gameViewKey(game: GameInfo): string {
  const view = game.library_view ?? (game.launcher === "PeliGames" ? "own" : "all");
  return `${view}:${game.launcher}:${game.path}`;
}

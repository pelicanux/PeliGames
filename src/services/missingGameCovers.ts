import { convertFileSrc } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";
import { fetchGameCoverByName, hasCustomCover } from "./customCovers";
import { rememberAutomaticCover } from "./automaticCovers";

/** Ignore searches made obsolete by a rename, removal or manual artwork choice. */
export async function findMissingGameCover(game: GameInfo, isCurrent: () => boolean): Promise<GameInfo | undefined> {
  const current = () => isCurrent() && !hasCustomCover(game);
  if (game.cover_url || !game.name.trim() || !current()) return;
  const remote = await fetchGameCoverByName(game.name);
  if (!remote || !current()) return;
  const saved = await rememberAutomaticCover(game, remote, current);
  if (!current()) return;
  const cover = saved.startsWith("file://") ? convertFileSrc(decodeURIComponent(new URL(saved).pathname)) : saved;
  return { ...game, automatic_cover_url: saved, cover_url: cover };
}

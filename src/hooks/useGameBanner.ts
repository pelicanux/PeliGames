import { useEffect, useState } from "react";
import type { GameInfo } from "../components/GameGrid";
import { fetchGameBanner } from "../services/gameBanners";
import { gameBannerKey } from "../services/gameBannerIdentity";

/** Banners found in this session, so a card mounted again shows its banner in the first frame. */
const found = new Map<string, string>();

/** The game's banner, and whether the search for it is over (`done`). */
export function useGameBannerState(game?: GameInfo, enabled = true): { url?: string; done: boolean } {
  const key = game && enabled ? gameBannerKey(game) : "";
  const [result, setResult] = useState<{ key: string; url?: string } | undefined>(() => key && found.has(key) ? { key, url: found.get(key) } : undefined);
  useEffect(() => {
    if (!game || !key) return;
    if (found.has(key)) { setResult({ key, url: found.get(key) }); return; }
    let active = true;
    void fetchGameBanner(game).then(url => {
      if (url) found.set(key, url);
      if (active) setResult({ key, url });
    });
    return () => { active = false; };
  }, [key]); // The key includes the store identity or search title, independent of launcher paths.
  if (!key) return { done: true };
  if (result?.key === key) return { url: result.url, done: true };
  return found.has(key) ? { url: found.get(key), done: true } : { done: false };
}

export function useGameBanner(game?: GameInfo, enabled = true): string | undefined {
  return useGameBannerState(game, enabled).url;
}

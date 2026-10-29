import { useEffect, useRef } from "react";
import type { GameInfo } from "../components/GameGrid";
import { automaticCoverKey } from "../services/automaticCovers";
import { hasCustomCover } from "../services/customCovers";
import { findMissingGameCover } from "../services/missingGameCovers";

/** Fill empty portraits without blocking the library; a corrected title gets a new search. */
export function useMissingGameCovers(games: GameInfo[], enabled: boolean, scope: "own" | "all") {
  const latest = useRef({ games, enabled, scope });
  latest.current = { games, enabled, scope };
  const mounted = useRef(false);
  const attempted = useRef(new Set<string>());
  const queues = useRef<Promise<unknown>[]>([Promise.resolve(), Promise.resolve()]);
  const nextQueue = useRef(0);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  useEffect(() => {
    if (!enabled) return;
    for (const entry of games) {
      const game = { ...entry, library_view: scope };
      if (game.cover_url || !game.name.trim() || hasCustomCover(game)) continue;
      const identity = automaticCoverKey(game);
      const key = JSON.stringify([identity, game.name.trim()]);
      if (attempted.current.has(key)) continue;
      attempted.current.add(key);
      const isCurrent = () => mounted.current && latest.current.enabled && latest.current.scope === scope
        && latest.current.games.some(item => automaticCoverKey(item) === identity
          && item.name.trim() === game.name.trim() && !item.cover_url);
      const slot = nextQueue.current++ % queues.current.length;
      const task = queues.current[slot].then(async () => {
        const found = await findMissingGameCover(game, isCurrent);
        if (found) window.dispatchEvent(new CustomEvent("gameCoverChanged", { detail: found }));
      }).catch(error => { console.warn("Could not find missing game artwork", error); });
      queues.current[slot] = task;
      void task.finally(() => { if (!isCurrent()) attempted.current.delete(key); });
    }
  }, [games, enabled, scope]);
}

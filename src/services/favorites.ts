import { useMemo, useSyncExternalStore } from "react";
import type { GameInfo } from "../components/GameGrid";
import { gameViewKey } from "./gameIdentity";
const storageKey = "peligames_favorites";
const changedEvent = "peligamesFavoritesChanged";
function snapshot() {
  try { return localStorage.getItem(storageKey) || "[]"; } catch { return "[]"; }
}
function parse(value: string): Set<string> {
  try {
    const entries: unknown = JSON.parse(value);
    return new Set(Array.isArray(entries) ? entries.filter((entry): entry is string => typeof entry === "string") : []);
  } catch { return new Set(); }
}
function subscribe(notify: () => void) {
  const storageChanged = (event: StorageEvent) => { if (!event.key || event.key === storageKey) notify(); };
  window.addEventListener(changedEvent, notify);
  window.addEventListener("storage", storageChanged);
  return () => { window.removeEventListener(changedEvent, notify); window.removeEventListener("storage", storageChanged); };
}
export function useFavorites() {
  const value = useSyncExternalStore(subscribe, snapshot, () => "[]");
  return useMemo(() => parse(value), [value]);
}
export function toggleFavorite(game: GameInfo) {
  const favorites = parse(snapshot()); const key = gameViewKey(game);
  if (favorites.has(key)) favorites.delete(key); else favorites.add(key);
  localStorage.setItem(storageKey, JSON.stringify([...favorites]));
  window.dispatchEvent(new Event(changedEvent));
}

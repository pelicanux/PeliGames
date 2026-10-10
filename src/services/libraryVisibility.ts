import { useMemo, useSyncExternalStore } from "react";
import type { GameInfo } from "../components/GameGrid";
import { gameViewKey } from "./gameIdentity";
const key = "peligames_removed_library_games";
const event = "peligamesLibraryVisibilityChanged";
function snapshot() { try { return localStorage.getItem(key) || "[]"; } catch { return "[]"; } }
function parse(value: string): Set<string> {
  try { const data = JSON.parse(value); return new Set(Array.isArray(data) ? data.filter(item => typeof item === "string") : []); }
  catch { return new Set(); }
}
function subscribe(notify: () => void) {
  const storage = (e: StorageEvent) => { if (!e.key || e.key === key) notify(); };
  window.addEventListener(event, notify); window.addEventListener("storage", storage);
  return () => { window.removeEventListener(event, notify); window.removeEventListener("storage", storage); };
}
export function useRemovedLibraryGames() {
  const value = useSyncExternalStore(subscribe, snapshot, () => "[]");
  return useMemo(() => parse(value), [value]);
}
export function canRemoveFromLibrary(game: GameInfo) {
  if (game.launcher === "Steam") return false;
  if (game.launcher === "PeliGames") return (game.library_view ?? "own") === "own";
  return ["Manual", "Custom"].includes(game.launcher);
}
export function setLibraryGameRemoved(game: GameInfo, removed: boolean) {
  if (removed && !canRemoveFromLibrary(game)) return;
  const entries = parse(snapshot());
  if (removed) entries.add(gameViewKey(game)); else entries.delete(gameViewKey(game));
  localStorage.setItem(key, JSON.stringify([...entries]));
  window.dispatchEvent(new Event(event));
}

/** Explicitly adding a folder again restores its manually discovered cards. */
export function restoreRemovedLibraryFolder(folder: string) {
  const entries = parse(snapshot());
  const root = folder.replace(/[\\/]$/, "");
  for (const launcher of ["Manual", "Custom"]) {
    const prefix = `all:${launcher}:`;
    for (const entry of entries) {
      if (!entry.startsWith(prefix)) continue;
      const path = entry.slice(prefix.length);
      if (path === root || path.startsWith(`${root}/`)) entries.delete(entry);
    }
  }
  localStorage.setItem(key, JSON.stringify([...entries]));
  window.dispatchEvent(new Event(event));
}

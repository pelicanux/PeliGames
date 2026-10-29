import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../components/GameGrid";
import { bannerSearchName, gameBannerKey, steamIdForBanner } from "./gameBannerIdentity";

type CachedBanner = { url: string | null; updated: number };
const CACHE_KEY = "peligames.gameBanners.v1";
const pending = new Map<string, Promise<string | undefined>>();
// Background searches never delay rendering and use at most two concurrent requests.
const queues: Promise<unknown>[] = [Promise.resolve(), Promise.resolve()];
let nextQueue = 0;
function readCache(): Record<string, CachedBanner> {
  try { const value = JSON.parse(localStorage.getItem(CACHE_KEY) || "{}"); return value && typeof value === "object" && !Array.isArray(value) ? value : {}; }
  catch { return {}; }
}
function remember(key: string, url: string | null) {
  const values = { ...readCache(), [key]: { url, updated: Date.now() } };
  const bounded = Object.fromEntries(Object.entries(values).sort((a,b) => b[1].updated-a[1].updated).slice(0,300));
  try { localStorage.setItem(CACHE_KEY, JSON.stringify(bounded)); } catch { /* The in-flight request still serves the current view. */ }
}
function displayUrl(url: string): string {
  return url.startsWith("file://") ? convertFileSrc(decodeURIComponent(new URL(url).pathname)) : url;
}
function loadsAsBanner(url: string): Promise<boolean> {
  return new Promise(resolve => {
    const picture = new Image();
    const finish = (ok: boolean) => { clearTimeout(timer); picture.onload = picture.onerror = null; resolve(ok); };
    const timer = setTimeout(() => finish(false), 8000);
    picture.onload = () => finish(picture.naturalWidth > picture.naturalHeight * 1.3);
    picture.onerror = () => finish(false);
    picture.src = url;
  });
}
async function findBanner(game: GameInfo, key: string): Promise<string | undefined> {
  const cached = readCache()[key];
  if (cached && typeof cached.updated === "number" && Date.now()-cached.updated < (cached.url ? 7*86400000 : 6*3600000)) {
    if (cached.url === null) return;
    if (typeof cached.url === "string") {
      try { const url = displayUrl(cached.url); if (await loadsAsBanner(url)) return url; } catch { /* Retry a missing cached file. */ }
    }
  }
  const preview = import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window;
  const accept = async (remote: string) => {
    let saved = remote;
    if (!preview) {
      try { saved = await invoke<string>("cache_automatic_game_cover", { url: remote, gameName: `${game.name}-banner` }); } catch { /* Remote artwork remains a fallback if local storage is unavailable. */ }
    }
    const url = displayUrl(saved);
    if (!(await loadsAsBanner(url))) return;
    remember(key, saved);
    return url;
  };
  const id = steamIdForBanner(game);
  if (id) {
    const direct = await accept(`https://cdn.cloudflare.steamstatic.com/steam/apps/${id}/library_hero.jpg`);
    if (direct) return direct;
  }
  try {
    const candidates = await invoke<string[]>("fetch_game_banners", { name: bannerSearchName(game), appId: id || null });
    for (const candidate of candidates) {
      const banner = await accept(candidate);
      if (banner) return banner;
    }
    remember(key, null);
  } catch { /* A temporarily unavailable catalog must not replace the existing cover. */ }
}
export function fetchGameBanner(game: GameInfo): Promise<string | undefined> {
  const key = gameBannerKey(game);
  const existing = pending.get(key);
  if (existing) return existing;
  const slot = nextQueue++ % queues.length;
  const result = queues[slot].then(() => findBanner(game, key)).catch(() => undefined);
  queues[slot] = result;
  pending.set(key, result);
  void result.finally(() => pending.delete(key));
  return result;
}

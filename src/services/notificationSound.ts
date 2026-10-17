import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";

const KEY = "peligames.matrix.notificationSound";
export interface SoundPreferences { enabled: boolean; volume: number }
export function normalizeSoundPreferences(value: Partial<SoundPreferences> | null): SoundPreferences {
  return { enabled: value?.enabled !== false, volume: typeof value?.volume === "number" && Number.isFinite(value.volume) ? Math.round(Math.max(0, Math.min(100, value.volume))) : 35 };
}
function read(): SoundPreferences {
  try { return normalizeSoundPreferences(JSON.parse(localStorage.getItem(KEY) || "null")); }
  catch { return normalizeSoundPreferences(null); }
}
let preferences = read();
const listeners = new Set<() => void>();
let lastPlayed = -Infinity;
export function setNotificationSoundPreferences(patch: Partial<SoundPreferences>) {
  preferences = normalizeSoundPreferences({ ...preferences, ...patch });
  try { localStorage.setItem(KEY, JSON.stringify(preferences)); } catch { /* Remains usable in memory. */ }
  listeners.forEach(listener => listener());
}
export function useNotificationSoundPreferences() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, () => preferences);
}
/** Kept for the callers: the native chime needs no unlocking by a user interaction. */
export async function unlockNotificationSound(): Promise<boolean> {
  return preferences.enabled && preferences.volume > 0;
}
/** A short two-note chiptune, synthesized natively (Rust): the webview's WebAudio could crash without GStreamer. */
export async function playNotificationSound(test = false): Promise<boolean> {
  if (!preferences.enabled || preferences.volume === 0) return false;
  if (!test && performance.now() - lastPlayed < 1500) return false;
  lastPlayed = performance.now();
  try { return await invoke<boolean>("play_notification_sound", { volume: preferences.volume }); }
  catch { return false; /* Visual notifications always remain. */ }
}

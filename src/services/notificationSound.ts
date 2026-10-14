import { useSyncExternalStore } from "react";

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
let audio: AudioContext | null = null, lastPlayed = -Infinity;
export function setNotificationSoundPreferences(patch: Partial<SoundPreferences>) {
  preferences = normalizeSoundPreferences({ ...preferences, ...patch });
  try { localStorage.setItem(KEY, JSON.stringify(preferences)); } catch { /* Remains usable in memory. */ }
  listeners.forEach(listener => listener());
}
export function useNotificationSoundPreferences() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, () => preferences);
}
function context() {
  if (!audio || audio.state === "closed") audio = new AudioContext();
  return audio;
}
/** Unlock audio on a real interaction, so later background mentions can beep. */
export async function unlockNotificationSound(): Promise<boolean> {
  if (!preferences.enabled || preferences.volume === 0) return false;
  try { const value = context(); if (value.state === "suspended") await value.resume(); return value.state === "running"; }
  catch { return false; /* Visual notifications always remain. */ }
}
/** A short two-note chiptune, synthesized locally; no audio files or downloads. */
export async function playNotificationSound(test = false): Promise<boolean> {
  if (!preferences.enabled || preferences.volume === 0) return false;
  if (!test && performance.now() - lastPlayed < 1500) return false;
  try {
    const value = context();
    // Avoid queuing old notifications until a future interaction unlocks audio.
    if (value.state !== "running") return false;
    lastPlayed = performance.now();
    const start = value.currentTime;
    for (const [offset, frequency] of [[0, 659.25], [0.115, 987.77]]) {
      const oscillator = value.createOscillator(), gain = value.createGain();
      oscillator.type = "square";
      oscillator.frequency.setValueAtTime(frequency, start + offset);
      gain.gain.setValueAtTime(0, start + offset);
      gain.gain.linearRampToValueAtTime(0.08 * preferences.volume / 100, start + offset + 0.006);
      gain.gain.exponentialRampToValueAtTime(0.0001, start + offset + 0.09);
      oscillator.connect(gain); gain.connect(value.destination);
      oscillator.onended = () => { oscillator.disconnect(); gain.disconnect(); };
      oscillator.start(start + offset); oscillator.stop(start + offset + 0.1);
    }
    return true;
  } catch { return false; }
}

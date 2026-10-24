// Volume and mute per person in calls, as in Discord's user menu. Only this
// listener hears the change; it is saved on this device and applied by the
// native mixer at once, in the running call and in later ones.
import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";

export type UserVolume = { volume: number; muted: boolean };
const KEY = "peligames.matrix.userVolumes";
const DEFAULT: UserVolume = { volume: 100, muted: false };

let volumes: Record<string, UserVolume> = (() => {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, Partial<UserVolume>>;
    return Object.fromEntries(Object.entries(saved).filter(([user]) => user.startsWith("@"))
      .map(([user, value]) => [user, { volume: Math.min(200, Math.max(0, Math.round(Number(value.volume ?? 100)) || 0)), muted: value.muted === true }]));
  } catch { return {}; }
})();
const listeners = new Set<() => void>();

const apply = (user: string, value: UserVolume) => invoke("matrix_call_user_volume", { user, volume: value.muted ? 0 : value.volume });

export function matrixUserVolume(user: string): UserVolume { return volumes[user] ?? DEFAULT; }

export async function setMatrixUserVolume(user: string, patch: Partial<UserVolume>) {
  const next = { ...matrixUserVolume(user), ...patch };
  volumes = { ...volumes };
  // The default needs no entry: the list only keeps people that were changed.
  if (next.volume === DEFAULT.volume && !next.muted) delete volumes[user]; else volumes[user] = next;
  try { localStorage.setItem(KEY, JSON.stringify(volumes)); } catch { /* Kept for this session only. */ }
  listeners.forEach(listener => listener());
  await apply(user, next);
}

/** Sends the saved volumes to the native mixer (on joining a call). */
export async function pushMatrixUserVolumes() {
  await Promise.all(Object.entries(volumes).map(([user, value]) => apply(user, value).catch(() => {})));
}

export function useMatrixUserVolume(user: string): UserVolume {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, () => matrixUserVolume(user));
}

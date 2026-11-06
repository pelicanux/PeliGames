import {useMemo, useSyncExternalStore} from "react";

export interface LibraryAppearance { coverSize: number; showRecent: boolean; }
export const defaultLibraryAppearance: Readonly<LibraryAppearance> = {coverSize:100, showRecent:true};
export const libraryCoverSize = {min:70, max:150, step:10} as const;
const key = "peligames.libraryAppearance";
const changed = "peligamesLibraryAppearanceChanged";
let memory = "{}", unavailable = false;

export function normalizeLibraryAppearance(value: unknown): LibraryAppearance {
  const data = value && typeof value === "object" ? value as Partial<LibraryAppearance> : {};
  const size = data.coverSize;
  return {
    coverSize: typeof size === "number" && Number.isFinite(size)
      ? Math.min(libraryCoverSize.max, Math.max(libraryCoverSize.min, Math.round(size / libraryCoverSize.step) * libraryCoverSize.step))
      : defaultLibraryAppearance.coverSize,
    showRecent: typeof data.showRecent === "boolean" ? data.showRecent : defaultLibraryAppearance.showRecent,
  };
}
function snapshot() {
  if (unavailable) return memory;
  try { return localStorage.getItem(key) ?? "{}"; } catch { return memory; }
}
function parse(raw: string) {
  try { return normalizeLibraryAppearance(JSON.parse(raw)); }
  catch { return normalizeLibraryAppearance(null); }
}
function subscribe(notify: () => void) {
  const storage = (event: StorageEvent) => { if (!event.key || event.key === key) notify(); };
  window.addEventListener(changed, notify);
  window.addEventListener("storage", storage);
  return () => {window.removeEventListener(changed, notify); window.removeEventListener("storage", storage);};
}
export function updateLibraryAppearance(patch: Partial<LibraryAppearance>) {
  memory = JSON.stringify(normalizeLibraryAppearance({...parse(snapshot()), ...patch}));
  try {localStorage.setItem(key, memory); unavailable = false;} catch {unavailable = true;}
  window.dispatchEvent(new Event(changed));
}
export function useLibraryAppearance() {
  const raw = useSyncExternalStore(subscribe, snapshot, () => "{}");
  return useMemo(() => parse(raw), [raw]);
}

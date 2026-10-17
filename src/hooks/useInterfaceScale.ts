import { useCallback, useLayoutEffect, useSyncExternalStore, type Dispatch, type SetStateAction } from "react";

export const MAX_INTERFACE_SCALE = 1.5;
const SCALE_KEY = "ui_scale";
const SCALE_EVENT = "peligames-interface-scale";
let memoryScale = 1;
const normalizeScale = (value: number) => Number.isFinite(value) ? Math.min(MAX_INTERFACE_SCALE, Math.max(0.6, Math.round(value * 10) / 10)) : 1;
function readScale() {
  try { return normalizeScale(Number(localStorage.getItem(SCALE_KEY) ?? 1)); }
  catch { return memoryScale; }
}
function subscribe(changed: () => void) {
  const sync = (event: StorageEvent) => { if (event.key === SCALE_KEY || event.key === null) changed(); };
  window.addEventListener("storage", sync);
  window.addEventListener(SCALE_EVENT, changed);
  return () => { window.removeEventListener("storage", sync); window.removeEventListener(SCALE_EVENT, changed); };
}

export function useInterfaceScale() {
  const scale = useSyncExternalStore(subscribe, readScale, () => 1);
  useLayoutEffect(() => {
    document.documentElement.style.fontSize = `${16 * scale}px`;
    document.documentElement.style.setProperty("--interface-scale", String(scale));
  }, [scale]);
  const setScale: Dispatch<SetStateAction<number>> = useCallback(value => {
    memoryScale = normalizeScale(typeof value === "function" ? value(readScale()) : value);
    try { localStorage.setItem(SCALE_KEY, String(memoryScale)); } catch { /* Keep the current window usable without storage. */ }
    // Only explicit changes write the preference; receiving windows never echo
    // older values back over a newer click in another window.
    window.dispatchEvent(new Event(SCALE_EVENT));
  }, []);
  return [scale, setScale] as const;
}

// The screen share is drawn by a native layer above the webview, not by the
// page: this places it over an element (the call stage or the mini player)
// and follows it while it moves. One element owns the layer at a time.
import { useEffect, useLayoutEffect, useSyncExternalStore, type RefObject } from "react";
import { invoke } from "@tauri-apps/api/core";

let owner: symbol | null = null;
// Menus of the page that open over the video: the native layer steps aside meanwhile.
let covers = 0;
export function coverNativeVideo() { covers++; let done = false; return () => { if (!done) { done = true; covers--; } }; }
const nativeVideoOverlays = new Set<RefObject<HTMLElement | null>>();
/** Reserve the menu's visible rectangle in the native layer, without hiding the stream. */
export function useNativeVideoOverlay(ref: RefObject<HTMLElement | null>) {
  useLayoutEffect(() => {
    nativeVideoOverlays.add(ref);
    return () => { nativeVideoOverlays.delete(ref); };
  }, [ref]);
}
let placed = "";
type VideoRect = { x: number; y: number; width: number; height: number; crt?: boolean; fill?: boolean; controls?: { x: number; y: number; width: number; height: number } | null };
function place(id: symbol, rect: VideoRect | null) {
  if (rect) {
    owner = id;
    const key = JSON.stringify(rect);
    if (key === placed) return;
    placed = key;
    void invoke("matrix_screen_view", { rect }).catch(() => {});
  } else if (owner === id) {
    owner = null;
    placed = "";
    void invoke("matrix_screen_view", { rect: null }).catch(() => {});
  }
}

/** Keeps the native video over `ref` while `active`; hidden otherwise. */
export function useNativeVideo(ref: RefObject<HTMLElement | null>, active: boolean, options?: { crt: boolean; fill: boolean; controls: RefObject<HTMLElement | null> }) {
  useEffect(() => {
    if (!active || "__PELI_UI_PREVIEW__" in window) return;
    const id = Symbol("native-video");
    let frame = 0;
    const follow = () => {
      const element = ref.current;
      const box = element?.getBoundingClientRect();
      // Hidden elements (display none, zero size) hide the video too.
      if (covers > 0 || !box || box.width < 2 || box.height < 2) place(id, null);
      else {
        const controls = options?.controls.current;
        const controlBoxes = [controls, ...Array.from(nativeVideoOverlays, overlay => overlay.current)]
          .filter((overlay): overlay is HTMLElement => Boolean(overlay) && Number(getComputedStyle(overlay!).opacity) > 0)
          .map(overlay => overlay.getBoundingClientRect())
          .filter(rect => rect.width > 0 && rect.height > 0 && rect.right > box.left && rect.left < box.right && rect.bottom > box.top && rect.top < box.bottom);
        // The backend already supports a transparent opening for floating controls.
        // Combine intersecting menus with it and clamp the opening to the video.
        const opening = controlBoxes.length ? {
          left: Math.max(box.left, Math.min(...controlBoxes.map(rect => rect.left)) - 3),
          top: Math.max(box.top, Math.min(...controlBoxes.map(rect => rect.top)) - 3),
          right: Math.min(box.right, Math.max(...controlBoxes.map(rect => rect.right)) + 3),
          bottom: Math.min(box.bottom, Math.max(...controlBoxes.map(rect => rect.bottom)) + 3),
        } : null;
        place(id, { x: Math.round(box.left), y: Math.round(box.top), width: Math.round(box.width), height: Math.round(box.height),
          crt: options?.crt ?? false, fill: options?.fill ?? false,
          controls: opening ? { x: Math.floor(opening.left - box.left), y: Math.floor(opening.top - box.top), width: Math.ceil(opening.right - opening.left), height: Math.ceil(opening.bottom - opening.top) } : null });
      }
      frame = requestAnimationFrame(follow);
    };
    follow();
    return () => { cancelAnimationFrame(frame); place(id, null); };
  }, [ref, active, options?.crt, options?.fill, options?.controls]);
}

export type ScreenFit = "fill" | "contain";
const SCREEN_FIT_KEY = "peligames.matrix.screenFit";
let screenFit: ScreenFit = (() => { try { return localStorage.getItem(SCREEN_FIT_KEY) === "contain" ? "contain" : "fill"; } catch { return "fill"; } })();
const fitListeners = new Set<() => void>();
export function setMatrixScreenFit(value: ScreenFit) {
  screenFit = value;
  try { localStorage.setItem(SCREEN_FIT_KEY, value); } catch { /* Keep this session's choice. */ }
  fitListeners.forEach(listener => listener());
}
export function useMatrixScreenFit() {
  return useSyncExternalStore(listener => { fitListeners.add(listener); return () => fitListeners.delete(listener); }, () => screenFit);
}

// While the call view shows the watched share, the mini player stays away.
let stageShown = false;
const listeners = new Set<() => void>();
export function setCallStageShown(shown: boolean) {
  if (stageShown === shown) return;
  stageShown = shown;
  listeners.forEach(listener => listener());
}
export function useCallStageShown() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, () => stageShown);
}
/** Asks the Matrix chat to open on the voice channel of the call. */
export const OPEN_MATRIX_CALL_EVENT = "peligamesOpenMatrixCall";
// A request made while the chat is closed is read when it mounts.
let callViewRequested = false;
export function requestCallView() { callViewRequested = true; window.dispatchEvent(new Event(OPEN_MATRIX_CALL_EVENT)); }
export function takeCallViewRequest() { const requested = callViewRequested; callViewRequested = false; return requested; }

// Full screen inside the Matrix frame: the watched share covers the whole
// terminal (never the launcher). The mini player's maximize opens it too.
let fullscreen = false;
const fullscreenListeners = new Set<() => void>();
export function setCallFullscreen(value: boolean) {
  if (fullscreen === value) return;
  fullscreen = value;
  fullscreenListeners.forEach(listener => listener());
}
export function useCallFullscreen() {
  return useSyncExternalStore(listener => { fullscreenListeners.add(listener); return () => fullscreenListeners.delete(listener); }, () => fullscreen);
}

// The screen share is drawn by a native layer above the webview, not by the
// page: this places it over an element (the call stage or the mini player)
// and follows it while it moves. One element owns the layer at a time.
import { useEffect, useSyncExternalStore, type RefObject } from "react";
import { invoke } from "@tauri-apps/api/core";

let owner: symbol | null = null;
// Menus of the page that open over the video: the native layer steps aside meanwhile.
let covers = 0;
export function coverNativeVideo() { covers++; let done = false; return () => { if (!done) { done = true; covers--; } }; }
let placed = "";
function place(id: symbol, rect: { x: number; y: number; width: number; height: number } | null) {
  if (rect) {
    owner = id;
    const key = `${rect.x}|${rect.y}|${rect.width}|${rect.height}`;
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
export function useNativeVideo(ref: RefObject<HTMLElement | null>, active: boolean) {
  useEffect(() => {
    if (!active || "__PELI_UI_PREVIEW__" in window) return;
    const id = Symbol("native-video");
    let frame = 0;
    const follow = () => {
      const element = ref.current;
      const box = element?.getBoundingClientRect();
      // Hidden elements (display none, zero size) hide the video too.
      if (covers > 0 || !box || box.width < 2 || box.height < 2) place(id, null);
      else place(id, { x: Math.round(box.left), y: Math.round(box.top), width: Math.round(box.width), height: Math.round(box.height) });
      frame = requestAnimationFrame(follow);
    };
    follow();
    return () => { cancelAnimationFrame(frame); place(id, null); };
  }, [ref, active]);
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

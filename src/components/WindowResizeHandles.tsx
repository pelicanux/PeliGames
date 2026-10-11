import { getCurrentWindow } from "@tauri-apps/api/window";

// Frameless Linux windows need explicit resize hit targets along their edges.
export function WindowResizeHandles() {
  if ("__PELI_UI_PREVIEW__" in window) return null;
  return <>{(["North", "South", "East", "West", "NorthEast", "NorthWest", "SouthEast", "SouthWest"] as const).map(direction => <div key={direction} aria-hidden="true" className={`window-resize-handle resize-${direction}`} onMouseDown={event => {
    if (event.button !== 0) return;
    event.preventDefault(); event.stopPropagation();
    void getCurrentWindow().startResizeDragging(direction).catch(console.error);
  }} />)}</>;
}

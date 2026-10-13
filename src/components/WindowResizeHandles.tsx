import { getCurrentWindow } from "@tauri-apps/api/window";
import { createPortal } from "react-dom";

// Frameless Linux windows need explicit resize hit targets along their edges.
export function WindowResizeHandles() {
  if ("__PELI_UI_PREVIEW__" in window) return null;
  // Escape #root/app-wrapper stacking contexts so dialogs cannot cover the edges.
  return createPortal(<div className="window-resize-layer" aria-hidden="true">{(["North", "South", "East", "West", "NorthEast", "NorthWest", "SouthEast", "SouthWest"] as const).map(direction => <div key={direction} data-tauri-drag-region="false" className={`window-resize-handle resize-${direction}`} onMouseDown={event => {
    if (event.button !== 0) return;
    event.preventDefault(); event.stopPropagation();
    void getCurrentWindow().startResizeDragging(direction).catch(console.error);
  }} />)}</div>, document.body);
}

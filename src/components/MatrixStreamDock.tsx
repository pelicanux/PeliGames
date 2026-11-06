// The mini player of a watched screen share, as in Discord: it floats over
// the launcher while the call view is not open, and can be dragged. Clicking
// the picture goes back to the call; the video is the same native layer.
import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n/I18nContext";
import { useMatrixVoice, watchMatrixScreen } from "../services/matrixVoice";
import { requestCallView, setCallFullscreen, useCallStageShown, useNativeVideo } from "../services/matrixScreenView";
import { OPEN_MATRIX_CHAT_EVENT } from "../services/matrixChat";

const WIDTH = 320, MARGIN = 16;
const TITLE_BAR_HEIGHT = 50;
const POSITION_KEY = "peligames.matrixDock";
function savedPosition() {
  try {
    const saved = JSON.parse(localStorage.getItem(POSITION_KEY) || "null") as { right: number; bottom: number } | null;
    if (saved && Number.isFinite(saved.right) && Number.isFinite(saved.bottom)) return saved;
  } catch { /* The default corner. */ }
  return { right: MARGIN, bottom: MARGIN };
}

export function openMatrixCallView(full = false) {
  setCallFullscreen(full);
  requestCallView();
  window.dispatchEvent(new Event(OPEN_MATRIX_CHAT_EVENT));
}

export function MatrixStreamDock() {
  const {t: tx} = useI18n();

  const voice = useMatrixVoice();
  const stageShown = useCallStageShown();
  const shown = Boolean(voice.watching) && !stageShown && (voice.phase === "connected" || voice.phase === "reconnecting");
  const video = useRef<HTMLDivElement>(null);
  const dock = useRef<HTMLElement>(null);
  const [position, setPosition] = useState(savedPosition);
  const drag = useRef<{ x: number; y: number; right: number; bottom: number } | null>(null);
  useNativeVideo(video, shown);
  const constrain = (value: { right: number; bottom: number }) => {
    const box = dock.current?.getBoundingClientRect();
    const maxRight = Math.max(MARGIN, window.innerWidth - (box?.width ?? WIDTH) - MARGIN);
    const maxBottom = Math.max(MARGIN, window.innerHeight - (box?.height ?? 220) - TITLE_BAR_HEIGHT - MARGIN);
    return { right: Math.min(Math.max(value.right, MARGIN), maxRight), bottom: Math.min(Math.max(value.bottom, MARGIN), maxBottom) };
  };
  // The window may have shrunk since the position was saved.
  useLayoutEffect(() => {
    if (!shown) return;
    const keep = () => setPosition(current => {
      const next = constrain(current);
      return next.right === current.right && next.bottom === current.bottom ? current : next;
    });
    keep();
    const observer = new ResizeObserver(keep);
    if (dock.current) observer.observe(dock.current);
    window.addEventListener("resize", keep);
    return () => { observer.disconnect(); window.removeEventListener("resize", keep); };
  }, [shown]);
  if (!shown) return null;
  const name = voice.watching === "local" ? (tx("ui-matrix-stream-dock", "your_screen")) : voice.participants.find(item => item.id === voice.watching)?.name ?? "";
  // In the body, like the Matrix window: above it, so it can be dragged there too.
  return createPortal(<aside ref={dock} className="matrix-stream-dock" style={{ right: position.right, bottom: position.bottom, width: WIDTH, maxWidth: `calc(100vw - ${MARGIN * 2}px)`, boxSizing: "border-box" }} aria-label={tx("ui-matrix-stream-dock", "s_stream", {"name": name})}>
    <header onPointerDown={event => {
      if (event.button !== 0 || (event.target as HTMLElement).closest("button")) return;
      // No text selection while dragging across the launcher.
      event.preventDefault();
      window.getSelection()?.removeAllRanges();
      document.body.classList.add("is-dragging-dock");
      event.currentTarget.setPointerCapture(event.pointerId);
      drag.current = { x: event.clientX, y: event.clientY, ...position };
    }} onPointerMove={event => {
      const start = drag.current;
      if (!start) return;
      setPosition(constrain({
        right: start.right - (event.clientX - start.x),
        bottom: start.bottom - (event.clientY - start.y),
      }));
    }} onPointerUp={() => {
      document.body.classList.remove("is-dragging-dock");
      if (!drag.current) return;
      drag.current = null;
      try { localStorage.setItem(POSITION_KEY, JSON.stringify(position)); } catch { /* Kept for this session only. */ }
    }} onLostPointerCapture={() => { document.body.classList.remove("is-dragging-dock"); drag.current = null; }}>
      <span className="matrix-call-live">{tx("ui-matrix-stream-dock", "live")}</span><b title={name}>{name}</b>
      <button type="button" title={tx("ui-matrix-stream-dock", "return_to_the_room_in_stage_mode")} aria-label={tx("ui-matrix-stream-dock", "return_to_the_room_in_stage_mode")} onClick={() => openMatrixCallView(false)}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M9 5H4v5M4 5l8 8"/><path d="M14 4h6v16H4v-6"/></svg>
      </button>
      <button type="button" title={tx("ui-matrix-stream-dock", "full_screen_in_matrix")} aria-label={tx("ui-matrix-stream-dock", "full_screen_in_matrix")} onClick={() => openMatrixCallView(true)}>⤢</button>
      <button type="button" title={tx("ui-matrix-stream-dock", "stop_watching")} aria-label={tx("ui-matrix-stream-dock", "stop_watching")} onClick={() => void watchMatrixScreen(null)}>✕</button>
    </header>
    <div ref={video} className="matrix-stream-dock-video" role="button" tabIndex={0} aria-label={tx("ui-matrix-stream-dock", "back_to_the_call")} onClick={() => openMatrixCallView()} onKeyDown={event => { if (event.key === "Enter") openMatrixCallView(); }}/>
  </aside>, document.body);
}

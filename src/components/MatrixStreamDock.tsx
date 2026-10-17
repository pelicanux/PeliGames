// The mini player of a watched screen share, as in Discord: it floats over
// the launcher while the call view is not open, and can be dragged. Clicking
// the picture goes back to the call; the video is the same native layer.
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n/I18nContext";
import { useMatrixVoice, watchMatrixScreen } from "../services/matrixVoice";
import { requestCallView, setCallFullscreen, useCallStageShown, useNativeVideo } from "../services/matrixScreenView";
import { OPEN_MATRIX_CHAT_EVENT } from "../services/matrixChat";

const WIDTH = 320, MARGIN = 16;
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
  const { language } = useI18n(); const pt = language === "pt";
  const voice = useMatrixVoice();
  const stageShown = useCallStageShown();
  const shown = Boolean(voice.watching) && !stageShown && (voice.phase === "connected" || voice.phase === "reconnecting");
  const video = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState(savedPosition);
  const drag = useRef<{ x: number; y: number; right: number; bottom: number } | null>(null);
  useNativeVideo(video, shown);
  // The window may have shrunk since the position was saved.
  useEffect(() => {
    if (!shown) return;
    const keep = () => setPosition(current => ({ right: Math.min(Math.max(current.right, 0), Math.max(window.innerWidth - WIDTH, 0)), bottom: Math.min(Math.max(current.bottom, 0), Math.max(window.innerHeight - 220, 0)) }));
    keep();
    window.addEventListener("resize", keep);
    return () => window.removeEventListener("resize", keep);
  }, [shown]);
  if (!shown) return null;
  const name = voice.watching === "local" ? (pt ? "Sua tela" : "Your screen") : voice.participants.find(item => item.id === voice.watching)?.name ?? "";
  // In the body, like the Matrix window: above it, so it can be dragged there too.
  return createPortal(<aside className="matrix-stream-dock" style={{ right: position.right, bottom: position.bottom, width: WIDTH }} aria-label={pt ? `Transmissão de ${name}` : `${name}'s stream`}>
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
      setPosition({
        right: Math.min(Math.max(start.right - (event.clientX - start.x), 0), Math.max(window.innerWidth - WIDTH, 0)),
        bottom: Math.min(Math.max(start.bottom - (event.clientY - start.y), 0), Math.max(window.innerHeight - 220, 0)),
      });
    }} onPointerUp={() => {
      document.body.classList.remove("is-dragging-dock");
      if (!drag.current) return;
      drag.current = null;
      try { localStorage.setItem(POSITION_KEY, JSON.stringify(position)); } catch { /* Kept for this session only. */ }
    }} onLostPointerCapture={() => { document.body.classList.remove("is-dragging-dock"); drag.current = null; }}>
      <span className="matrix-call-live">{pt ? "AO VIVO" : "LIVE"}</span><b title={name}>{name}</b>
      <button type="button" title={pt ? "Tela cheia no Matrix" : "Full screen in Matrix"} aria-label={pt ? "Tela cheia no Matrix" : "Full screen in Matrix"} onClick={() => openMatrixCallView(true)}>⤢</button>
      <button type="button" title={pt ? "Parar de assistir" : "Stop watching"} aria-label={pt ? "Parar de assistir" : "Stop watching"} onClick={() => void watchMatrixScreen(null)}>✕</button>
    </header>
    <div ref={video} className="matrix-stream-dock-video" role="button" tabIndex={0} aria-label={pt ? "Voltar para a chamada" : "Back to the call"} onClick={() => openMatrixCallView()} onKeyDown={event => { if (event.key === "Enter") openMatrixCallView(); }}/>
  </aside>, document.body);
}

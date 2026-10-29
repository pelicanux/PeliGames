// Frames the page actually presents, measured with requestAnimationFrame so the
// number means the same on every web engine. Developer mode toggles it; it costs
// nothing while hidden because the frame loop only runs while it is shown.

const STORAGE_KEY = "peligames.frame_rate_meter";
const WINDOW_MS = 2000;

export interface FrameStats { fps: number; worstMs: number }

/** Frame rate and slowest frame of the timestamps (ms) inside the last window. */
export function frameStats(timestamps: readonly number[], windowMs = WINDOW_MS): FrameStats {
  if (timestamps.length < 2) return { fps: 0, worstMs: 0 };
  const end = timestamps[timestamps.length - 1];
  const recent = timestamps.filter(time => end - time <= windowMs);
  if (recent.length < 2) return { fps: 0, worstMs: 0 };
  let worstMs = 0;
  for (let i = 1; i < recent.length; i++) worstMs = Math.max(worstMs, recent[i] - recent[i - 1]);
  const span = recent[recent.length - 1] - recent[0];
  return { fps: Math.round(((recent.length - 1) * 1000) / span), worstMs: Math.round(worstMs * 10) / 10 };
}

function remembered(): boolean {
  try { return localStorage.getItem(STORAGE_KEY) === "1"; } catch { return false; }
}
function remember(shown: boolean) {
  try { localStorage.setItem(STORAGE_KEY, shown ? "1" : "0"); } catch { /* per-viewer convenience only */ }
}

let label: HTMLDivElement | null = null;
let frame = 0;
let timestamps: number[] = [];
let lastPaint = 0;

function tick(time: number) {
  timestamps.push(time);
  while (timestamps.length && time - timestamps[0] > WINDOW_MS) timestamps.shift();
  if (label && time - lastPaint >= 500) {
    const { fps, worstMs } = frameStats(timestamps);
    label.textContent = `${fps} FPS · pior quadro ${worstMs} ms`;
    lastPaint = time;
  }
  frame = requestAnimationFrame(tick);
}

export function frameRateMeterShown(): boolean {
  return label !== null;
}

/** Shows or hides the meter (Settings → Logs and more → Developer mode). */
export function setFrameRateMeter(shown: boolean) {
  remember(shown);
  if (shown === Boolean(label)) return;
  if (!shown) {
    cancelAnimationFrame(frame);
    label?.remove();
    label = null;
    return;
  }
  label = document.createElement("div");
  label.setAttribute("aria-hidden", "true");
  Object.assign(label.style, {
    position: "fixed", top: "8px", right: "8px", zIndex: "2147483647", pointerEvents: "none",
    padding: "4px 8px", borderRadius: "6px", background: "rgba(0,0,0,.72)", color: "#7CFFB2",
    font: "600 12px/1.2 ui-monospace, monospace",
  });
  label.textContent = "… FPS";
  document.body.appendChild(label);
  timestamps = [];
  frame = requestAnimationFrame(tick);
}

/** Restores the meter when it was left on. */
export function installFrameRateMeter() {
  if (remembered()) setFrameRateMeter(true);
}

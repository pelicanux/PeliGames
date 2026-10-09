import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { ModalSurface } from "../../components/ModalSurface";
import { useNexusText } from "./text";
export function NexusLogsModal({ gameId, onClose }: { gameId: string; onClose: () => void }) {
  const text = useNexusText();
  const [tab, setTab] = useState<"mods" | "current" | "previous">("mods");
  const [content, setContent] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    const refresh = () => void invoke<string>(tab === "mods" ? "read_nexus_mod_logs" : "read_nexus_logs", { gameId, previous: tab === "previous" }).then(result => { if (active) {
      setContent(tab === "mods" ? result.replace(/modified_unix=(\d+)/g, (_, seconds: string) => Number(seconds) ? new Date(Number(seconds) * 1000).toLocaleString() : "—") : result);
      setError("");
    } }).catch(e => { if (active) setError(String(e)); });
    setContent(""); setError(""); refresh();
    const timer = window.setInterval(refresh, 2500);
    return () => { active = false; window.clearInterval(timer); };
  }, [gameId, tab]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopImmediatePropagation(); onClose(); } };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [onClose]);
  return createPortal(<div className="modal-overlay launcher-logs-overlay" onMouseDown={e => { if (e.target === e.currentTarget) onClose(); }}>
    <ModalSurface role="dialog" aria-modal="true" aria-labelledby="nexus-log-title" className="modal-content dialog-glass game-logs-dialog" header={<h2 id="nexus-log-title">{text.logs}</h2>} onDismiss={onClose}>
      <div className="nexus-tabs">{(["mods", "current", "previous"] as const).map(value => <button key={value} className={`btn btn-secondary ${tab === value ? "active" : ""}`} onClick={() => setTab(value)} aria-pressed={tab === value}>{value === "mods" ? text.modLoadingLogs : value === "current" ? text.currentLog : text.previousLog}</button>)}</div>
      {tab === "mods" && <p className="nexus-hint">{text.modLoadingHint}</p>}
      {error && <p role="alert">{error}</p>}<pre className="game-log-output" tabIndex={0}>{content || text.loading}</pre>
    </ModalSurface>
  </div>, document.body);
}

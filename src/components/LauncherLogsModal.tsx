import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { usePreferencesText } from "./preferences/labels";

export function LauncherLogsModal({ onClose }: { onClose: () => void }) {
  const text = usePreferencesText();
  const [log, setLog] = useState({ path: "", content: "" });
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let active = true;
    setLoading(true);
    invoke<{ path: string; content: string }>("get_launcher_log")
      .then(result => { if (active) { setLog(result); setError(""); } })
      .catch(failure => { if (active) setError(String(failure)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [revision]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.stopImmediatePropagation(); onClose(); }
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [onClose]);
  return createPortal(<div className="modal-overlay launcher-logs-overlay" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <ModalSurface role="dialog" aria-modal="true" aria-labelledby="launcher-log-title" className="modal-content dialog-glass game-logs-dialog" header={<h2 id="launcher-log-title">{text.launcherLogs}</h2>} onDismiss={onClose}>
      <div className="launcher-log-toolbar"><code>{log.path || "~/.config/PeliGames/launcher.log"}</code><button type="button" disabled={loading} onClick={() => setRevision(value => value + 1)}><MenuIcon name="repair"/>{text.refreshLogs}</button></div>
      {error && <p role="alert">{error}</p>}
      <pre className="game-log-output" tabIndex={0} aria-busy={loading}>{log.content || (loading ? text.loadingLogs : error ? "" : text.emptyLogs)}</pre>
    </ModalSurface>
  </div>, document.body);
}

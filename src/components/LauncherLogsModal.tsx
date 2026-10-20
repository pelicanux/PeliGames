import "./LauncherLogsModal.css";
import { OpenLogFolderButton } from "./DetachedLogWindow";
import { useEffect, useState, useRef } from "react";
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
  const output=useRef<HTMLPreElement>(null),follow=useRef(true);
  useEffect(() => {
    let active=true;let timer:ReturnType<typeof setTimeout>;
    setLoading(true);
    const refresh=()=>{
      void invoke<{path:string;content:string}>("get_launcher_log")
        .then(result=>{if(active){const element=output.current;follow.current=!element||element.scrollHeight-element.scrollTop-element.clientHeight<32;setLog(result);setError("");}})
        .catch(failure=>{if(active)setError(String(failure));})
        .finally(()=>{if(active){setLoading(false);timer=setTimeout(refresh,3000);}});
    };
    refresh();return()=>{active=false;clearTimeout(timer);};
  },[revision]);
  useEffect(()=>{if(follow.current&&output.current)output.current.scrollTop=output.current.scrollHeight;},[log.content]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.stopImmediatePropagation(); onClose(); }
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [onClose]);
  return createPortal(<div className="modal-overlay launcher-logs-overlay" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <ModalSurface role="dialog" aria-modal="true" aria-labelledby="launcher-log-title" className="modal-content dialog-glass game-logs-dialog launcher-log-dialog" header={<h2 id="launcher-log-title">{text.launcherLogs}</h2>} onDismiss={onClose}>
      <div className="launcher-log-toolbar"><code>{log.path || "~/.config/PeliGames/launcher.log"}</code><button type="button" disabled={loading} onClick={() => setRevision(value => value + 1)}><MenuIcon name="repair"/>{text.refreshLogs}</button></div>
      <p className="launcher-log-description">{text.launcherLogDescription}</p>
      <OpenLogFolderButton kind="launcher"/>
      {error && <p role="alert">{error}</p>}
      <pre ref={output} className="game-log-output" aria-label={text.launcherLogs} tabIndex={0} aria-busy={loading}>{log.content || (loading ? text.loadingLogs : error ? "" : text.emptyLogs)}</pre>
      <small className="launcher-log-summary">{text.launcherLogRetention}</small>
    </ModalSurface>
  </div>, document.body);
}

import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { ModalSurface } from "../../components/ModalSurface";
import { OpenLogFolderButton, DetachedLogWindow, dragLogWindow, LogWindowActions, LogOutput, LogResizeHandles } from "../../components/DetachedLogWindow";
import { useNexusText } from "./text";
export function NexusLogsModal({ gameId, onClose, standalone = false }: { gameId: string; onClose: () => void; standalone?: boolean }) {
  return standalone ? <NexusLogsContent gameId={gameId} onClose={onClose} /> : <DetachedLogWindow kind="nexus" target={gameId} title="Nexus Mods" onClose={onClose} />;
}
function NexusLogsContent({ gameId, onClose }: { gameId: string; onClose: () => void }) {
  const text = useNexusText();
  // The current launch log is cheap to read; mod/crash logs are scanned only on demand.
  const [tab, setTab] = useState<"mods" | "current" | "previous">("current");
  const [content, setContent] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [refreshVersion, setRefreshVersion] = useState(0);
  useEffect(() => {
    const deleted = () => setRefreshVersion(value => value + 1);
    window.addEventListener("gameLogsDeleted", deleted);
    return () => window.removeEventListener("gameLogsDeleted", deleted);
  }, []);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const refresh = () => void invoke<string>(tab === "mods" ? "read_nexus_mod_logs" : "read_nexus_logs", { gameId, previous: tab === "previous" }).then(result => { if (active) {
      setContent(tab === "mods" ? result.replace(/modified_unix=(\d+)/g, (_, seconds: string) => Number(seconds) ? new Date(Number(seconds) * 1000).toLocaleString() : "—") : result);
      setError("");
    } }).catch(e => { if (active) setError(String(e)); }).finally(() => { if (active) { setLoading(false); timer = setTimeout(refresh, tab === "mods" ? 6000 : 2500); } });
    setContent(""); setError(""); setLoading(true); refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [gameId, tab, refreshVersion]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopImmediatePropagation(); onClose(); } };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [onClose]);
  return createPortal(<div className="detached-log-page">
    <ModalSurface role="dialog" aria-modal="false" aria-labelledby="nexus-log-title" className="modal-content dialog-glass game-logs-dialog" onHeaderMouseDown={dragLogWindow} hideClose headerActions={<LogWindowActions onClose={onClose} />} header={<h2 id="nexus-log-title">{text.logs}</h2>} onDismiss={onClose}>
      <div className="nexus-tabs">{(["current", "previous", "mods"] as const).map(value => <button key={value} className={`btn btn-secondary ${tab === value ? "active" : ""}`} onClick={() => setTab(value)} aria-pressed={tab === value}>{value === "mods" ? text.modLoadingLogs : value === "current" ? text.currentLog : text.previousLog}</button>)}</div>
      <OpenLogFolderButton kind="nexus" target={gameId}/>
      {tab === "mods" && <p className="nexus-hint">{text.modLoadingHint}</p>}
      {error && <p role="alert">{error}</p>}
      {(!error || content) && <LogOutput content={content || (loading ? text.loading : text.noLogs)} />}
    </ModalSurface>
    <LogResizeHandles />
  </div>, document.body);
}

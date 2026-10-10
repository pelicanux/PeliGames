import { nexusFileCategory } from "./fileGroups";
import { MenuIcon } from "../../components/MenuIcon";
import { NexusFileChoices } from "./NexusFileChoices";
import { showDownloadFeedback } from "./downloadFeedback";
import { Fragment, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { NexusPopup } from "./NexusPopup";
import { plainNexusText } from "./nexusLinks";
import { collectQueue, type QueueDetails, type Entry, type External } from "./queueGraph";
export type { QueueDetails } from "./queueGraph";
import { supportedNexusFile } from "./archiveFormats";
import type { NexusGame } from "./useNexusWorkspace";
import { useNexusText } from "./text";
export function NexusDownloadQueue({ game, domain, modId, details, fileId, onClose, onStarted, embedded = false }: {
  game: NexusGame; domain: string; modId: number; details: QueueDetails; fileId?: number;
  onClose: () => void; onStarted: () => void; embedded?: boolean;
}) {
  const text = useNexusText();
  const [selectedFiles, setSelectedFiles] = useState<Record<number, number>>({});
  const [entries, setEntries] = useState<Entry[]>([]), [external, setExternal] = useState<External[]>([]);
  const [optionalFiles, setOptionalFiles] = useState<Record<number, number[]>>({});
  const [busy, setBusy] = useState(true), [error, setError] = useState("");
  const mounted = useRef(true), starting = useRef(false);
  useEffect(() => {
    let active = true; mounted.current = true; setBusy(true);
    void collectQueue({ game, domain, modId, details, fileId, selectedFiles, active: () => active,
      fetchDetails: (id, selectedFile) => invoke<QueueDetails>("get_nexus_catalog_mod", { gameDomain: domain, modId: id, fileId: selectedFile }),
      limitWarning: text.queueLimit, incompleteWarning: text.requirementsUnknown,
    }).then(result => { if (active) { setEntries(result.entries); if (fileId && details.files.some(file => file.file_id === fileId && nexusFileCategory(file) === 3 && supportedNexusFile(file.file_name))) setOptionalFiles({ [modId]: [fileId] }); setExternal(result.external); setError(result.warnings.join("\n")); setBusy(false); } });
    return () => { active = false; mounted.current = false; };
  }, [game.id, domain, modId, details, fileId, selectedFiles]);
  const selected = entries.filter(e => e.checked && !e.installed);
  const optional = entries.flatMap(entry => (entry.details?.files || []).filter(file => nexusFileCategory(file) === 3 && supportedNexusFile(file.file_name) && optionalFiles[entry.id]?.includes(file.file_id)).map(file => ({ domain, mod_id: entry.id, file_id: file.file_id })));
  const downloadCount = selected.length + optional.length;
  const ready = downloadCount > 0 && downloadCount <= 16 && selected.every(e => e.file && e.details && !e.error);
  const start = async () => {
    if (starting.current || busy || !ready || game.running) return;
    starting.current = true; setBusy(true); setError("");
    showDownloadFeedback({ phase: "preparing" });
    try {
      await invoke("queue_nexus_downloads", { gameId: game.id, files: [...selected.map(e => ({ domain, mod_id: e.id, file_id: e.file })), ...optional] });
      showDownloadFeedback({ phase: "ready" });
      if (mounted.current) onStarted();
    } catch (e) { showDownloadFeedback({ phase: "error", error: String(e) }); if (mounted.current) setError(String(e)); }
    finally { starting.current = false; if (mounted.current) setBusy(false); }
  };
  return <NexusPopup embedded={embedded} className="nexus-queue-popup" title={text.queueTitle} onClose={onClose} footer={<button className="btn btn-primary" disabled={busy || !ready || game.running} onClick={() => void start()}>{text.queueDownload} ({downloadCount})</button>}>
    {busy && <p role="status" className="nexus-hint">{text.loading}</p>}
    {downloadCount > 16 && <p role="alert" className="nexus-requirements-warning">{text.queueLimit}</p>}
    {error && <p role="alert" className="nexus-requirements-warning">{error}</p>}
    <div className="nexus-queue-list">{entries.map((entry, index) => <Fragment key={entry.id}><article>
      <div className="nexus-queue-entry-heading"><label className="nexus-queue-choice" title={entry.parent ? `${text.queueRequiredBy}: ${entry.parent}` : undefined}><input type="checkbox" checked={entry.checked && !entry.installed} disabled={busy || entry.installed} onChange={e => setEntries(values => values.map(v => v.id === entry.id ? { ...v, checked: e.target.checked } : v))} /><strong>{plainNexusText(entry.name)}</strong><small className="nexus-queue-badge">{index === 0 ? text.queuePrimary : /\boptional\b|opcional/i.test(entry.notes) ? text.queueOptional : text.queueDependency}</small></label>{entry.notes && <span className="nexus-queue-note" title={plainNexusText(entry.notes)} aria-label={plainNexusText(entry.notes)}><MenuIcon name="info" /></span>}</div>
      {entry.installed && <small className="nexus-status">{text.queueAlreadyInstalled}</small>}
      {entry.error && <p className="nexus-error">{entry.error}</p>}
      {!entry.installed && entry.checked && entry.details && <NexusFileChoices files={entry.details.files.filter(f => supportedNexusFile(f.file_name) && nexusFileCategory(f) !== 3)} name={`queue-file-${entry.id}`} modName={entry.name} selected={entry.file} disabled={busy} onSelect={file => setSelectedFiles(values => ({ ...values, [entry.id]: file }))} />}
    </article>{entry.details?.files.some(file => nexusFileCategory(file) === 3 && supportedNexusFile(file.file_name)) && <article className="nexus-queue-optionals">
      <div className="nexus-queue-entry-heading"><strong>{plainNexusText(entry.name)}</strong><small className="nexus-queue-badge">{text.queueOptionals}</small></div>
      <div className="nexus-queue-files" role="group" aria-label={`${text.optionalFiles}: ${entry.name}`}>{entry.details.files.filter(file => nexusFileCategory(file) === 3 && supportedNexusFile(file.file_name)).map(file => <label className={`nexus-queue-file ${optionalFiles[entry.id]?.includes(file.file_id) ? "selected" : ""}`} key={file.file_id}>
        <input type="checkbox" aria-label={`${file.name} · ${file.version}`} checked={Boolean(optionalFiles[entry.id]?.includes(file.file_id))} disabled={busy} onChange={event => { const checked = event.target.checked; setOptionalFiles(values => ({...values, [entry.id]: checked ? [...(values[entry.id] || []), file.file_id] : (values[entry.id] || []).filter(id => id !== file.file_id)})); }} />
        <span className="nexus-queue-file-info"><strong>{plainNexusText(file.name)}</strong><small>{file.version} · {file.size < 1024 ? `${file.size.toFixed(1)} KB` : `${(file.size / 1024).toFixed(1)} MB`}</small>{file.description?.trim() && <small>{plainNexusText(file.description)}</small>}</span>
      </label>)}</div>
    </article>}</Fragment>)}</div>
    {!!external.length && <><h4>{text.externalRequirement}</h4><div className="nexus-requirement-list">{external.map((item, i) => <article key={i}><div><div className="nexus-queue-external-heading"><strong>{plainNexusText(item.name)}</strong><small className="nexus-queue-badge">{/\boptional\b|opcional/i.test(item.notes) ? `${text.queueOptional} · ` : ""}{item.kind === "dlc" ? text.dlcRequirement : item.kind === "nexus" ? text.queueOtherGame : text.queueExternal}</small></div><p className="nexus-file-description">{plainNexusText(item.notes)}</p></div>{item.installed && <small className="nexus-status">{text.queueAlreadyInstalled}</small>}{item.url && !item.installed && <button className="btn btn-secondary" disabled={busy} onClick={() => void openUrl(item.url!).catch(() => setError(text.accountBrowserError))}>{text.externalDownload}</button>}</article>)}</div></>}
  </NexusPopup>;
}

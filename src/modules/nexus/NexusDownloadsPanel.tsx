import { useEffect, useRef, useState } from "react";
import { nexusDownloadFeedback, type DownloadFeedback } from "./downloadFeedback";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl, openNexusBrowser } from "../../services/browser";
import { NexusPopup } from "./NexusPopup";
import { NexusRequirements, type Requirements } from "./NexusRequirements";
import { HoverTooltip } from "../../components/HoverTooltip";
import { MenuIcon } from "../../components/MenuIcon";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";
export interface NexusDownload { id: string; domain: string; mod_id: number; file_id: number; game_id?: string; name: string; status: "preparing" | "queued" | "authorizing" | "waiting" | "downloading" | "importing" | "installing" | "installed" | "imported" | "cancelled" | "error"; received: number; total?: number; error?: string; auto_install?: boolean; mod_entry_id?: string; requirements?: Requirements }
export function NexusDownloadsPanel({ games, onViewMod, buttonClassName = "btn btn-secondary" }: { buttonClassName?: string; games: NexusGame[]; onViewMod: (gameId: string, modId: string) => void }) {
  const text = useNexusText();
  const [opened, setOpened] = useState(false);
  const [selectedJob, setSelectedJob] = useState<string | null>(null);
  const [jobs, setJobs] = useState<NexusDownload[]>([]);
  const [error, setError] = useState("");
  const [registered, setRegistered] = useState(false);
  const [busy, setBusy] = useState(false);
  const [preparing, setPreparing] = useState(false);
  const seen = useRef(new Map<string, NexusDownload["status"]>());
  useEffect(() => {
    let active = true, querying = false;
    const refresh = async () => {
      if (querying) return; querying = true;
      try { const rows = await invoke<NexusDownload[]>("list_nexus_downloads"); if (active) {
        if (rows.some(job => (["authorizing", "waiting", "downloading", "error"] as string[]).includes(job.status) && seen.current.get(job.id) !== job.status && (job.status !== "error" || seen.current.has(job.id)))) setOpened(true);
        seen.current = new Map(rows.map(job => [job.id, job.status]));
        setJobs(rows);
      } }
      catch (e) { if (active) setError(String(e)); } finally { querying = false; }
    };
    const feedback = (event: Event) => {
      const detail = (event as CustomEvent<DownloadFeedback>).detail;
      setOpened(true); setSelectedJob(null); setPreparing(detail.phase === "preparing"); setError(detail.error || "");
      void refresh();
    };
    window.addEventListener(nexusDownloadFeedback, feedback);
    void refresh(); const timer = window.setInterval(() => void refresh(), 1000);
    return () => { active = false; window.clearInterval(timer); window.removeEventListener(nexusDownloadFeedback, feedback); };
  }, []);
  const run = async (command: string, args?: Record<string, unknown>) => {
    if (busy) return; setBusy(true); setError("");
    try { await invoke(command, args); if (command === "register_nexus_handler") setRegistered(true); }
    catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const activeJobs = jobs.filter(job => ["preparing", "queued", "authorizing", "waiting", "downloading", "importing", "installing"].includes(job.status));
  const requirementJob = jobs.find(job => job.id === selectedJob);
  return <><div className="nexus-downloads-toolbar"><HoverTooltip text={text.downloadsHint}><button type="button" className={buttonClassName} onClick={() => setOpened(true)}><MenuIcon name="download" />{text.downloads}{activeJobs.length ? ` (${activeJobs.length})` : ""}</button></HoverTooltip></div>
    {opened && <NexusPopup title={requirementJob?.requirements ? text.requirements : text.downloads} viewKey={requirementJob?.requirements ? `requirements-${requirementJob.id}` : "downloads"} onBack={requirementJob?.requirements ? () => setSelectedJob(null) : undefined} className="nexus-downloads-popup" headerActions={<HoverTooltip showOnFocus={false} text={registered ? text.nxmRegistered : text.nxmHint} anchorClassName="nexus-downloads-header-action"><button type="button" className="btn btn-secondary" aria-label={text.registerNxm} disabled={busy} onClick={() => void run("register_nexus_handler")}><MenuIcon name="platform" /><span>{text.registerNxm}</span></button></HoverTooltip>} onClose={() => { setOpened(false); setSelectedJob(null); }}><section className="nexus-downloads" hidden={Boolean(requirementJob?.requirements)}>

    {error && <p role="alert" className="nexus-error">{error}</p>}
    <div className="nexus-browser-login"><button type="button" className="btn btn-secondary" onClick={() => void openNexusBrowser().catch(error => setError(typeof error === "string" ? error : error instanceof Error ? error.message : text.accountBrowserError))}><MenuIcon name="platform" />{text.internalNexusLogin}</button><p className="nexus-hint">{text.internalBrowserHint}</p></div>
    {preparing && <div className="nexus-download-wait" role="status"><div className="nexus-loading-emblem"><span className="nexus-loading-ring" /><img src="/peligames.svg" alt="" /></div><strong>{text.preparing}</strong></div>}
    {!preparing && activeJobs.some(job => job.status === "authorizing") && <div className="nexus-download-wait" role="status"><div className="nexus-loading-emblem"><span className="nexus-loading-ring" /><img src="/peligames.svg" alt="" /></div><strong>{text.browserConfirmation}</strong><p className="nexus-hint">{text.browserConfirmationHint}</p></div>}
    {!!jobs.length && <div className="nexus-mod-list" aria-live="polite">{jobs.slice().reverse().map(job => <article className="nexus-mod-row nexus-download-job" key={job.id}><div><strong>{job.name}</strong><small>{games.find(g => g.id === job.game_id)?.game.name || job.domain} · {text[job.status]}</small>
      {job.status === "downloading" && <><progress aria-label={text.downloading} {...(job.total ? { max: job.total, value: job.received } : {})} /><small>{(job.received / 1048576).toFixed(1)} MB{job.total ? ` / ${(job.total / 1048576).toFixed(1)} MB` : ""}</small></>}
      {job.status === "authorizing" && <><p className="nexus-hint">{(job.auto_install ? text.dependencyAuthorize : text.queueAuthorize)}</p></>}
      {job.status === "authorizing" && import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window && new URLSearchParams(location.search).has("demo") && <button type="button" className="btn btn-primary" onClick={() => void run("preview_confirm_nexus_download")}>Simular confirmação no navegador</button>}
      {job.status === "waiting" && <><p className="nexus-hint">{text.chooseDownloadGame}</p>{games.filter(g => (g.adapter || g.nexus_domain) === job.domain).map(g => <button type="button" key={g.id} className="btn btn-secondary" disabled={busy} onClick={() => void run("start_nexus_download", { downloadId: job.id, gameId: g.id })}>{g.game.name}</button>)}{!games.some(g => (g.adapter || g.nexus_domain) === job.domain) && <p className="nexus-hint">{text.addDownloadGame}</p>}</>}

      {job.error && <><p className="nexus-error">{job.error}</p>{job.mod_entry_id && <p className="nexus-hint">{text.dependencyInstallFailed}</p>}</>}{job.status === "imported" && <small>{text.downloadImported}</small>}
    </div><div className="nexus-download-job-actions">{(job.status === "authorizing" || (job.status === "error" && !job.received && !job.mod_entry_id)) && <><button type="button" className="btn btn-primary" disabled={busy} onClick={() => void run("retry_nexus_download", { downloadId: job.id, external: false })}><MenuIcon name="download" />{job.status === "error" ? text.tryAgain : text.downloadNexus}</button><button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void run("retry_nexus_download", { downloadId: job.id, external: true })}><MenuIcon name="platform" />{text.externalBrowser}</button></>}{job.requirements && <button type="button" className="btn btn-secondary" onClick={() => setSelectedJob(job.id)}><MenuIcon name="puzzle" />{text.requirements}{job.requirements.items.length ? ` (${job.requirements.items.length})` : ""}</button>}{["imported", "installed"].includes(job.status) && job.game_id && job.mod_entry_id && <button type="button" className="btn btn-primary" onClick={() => { setOpened(false); setSelectedJob(null); onViewMod(job.game_id!, job.mod_entry_id!); }}><MenuIcon name="eye" />{text.viewDownloadedMod}</button>}{(job.status === "queued" || job.status === "error" || job.status === "authorizing" || job.status === "waiting" || job.status === "downloading") && <button type="button" className="btn btn-secondary nexus-download-cancel" disabled={busy} onClick={() => void run("cancel_nexus_download", { downloadId: job.id })}>{text.cancel}</button>}</div></article>)}</div>}
      {!jobs.length && !preparing && <p className="nexus-hint">{text.noDownloads}</p>}
    </section>
    {requirementJob?.requirements && <NexusPopup embedded title={text.requirements} onClose={() => setSelectedJob(null)}><h3>{requirementJob.name}</h3><NexusRequirements game={games.find(g => g.id === requirementJob.game_id)} requirements={requirementJob.requirements} onOpen={url => void openUrl(url).catch(() => setError(text.accountBrowserError))} /><button type="button" className="btn btn-secondary" onClick={() => void openUrl(`https://www.nexusmods.com/${requirementJob.domain}/mods/${requirementJob.mod_id}`).catch(() => setError(text.accountBrowserError))}>{text.modPage}</button></NexusPopup>}
    </NexusPopup>}

  </>;
}

import type {Catalog} from "../../i18n/engine";
import { ComponentDownloadProgress } from "../ComponentDownloadProgress";
import { formatTransferBytes } from "../../services/downloadProgress";
import { invoke } from "@tauri-apps/api/core";
import { notifyError } from "../../services/notifications";
import { useEffect, useRef, useState } from "react";
import { ConfirmModal } from "../ConfirmModal";
import { MenuIcon } from "../MenuIcon";
import { useI18n } from "../../i18n/I18nContext";
import { checkRunner, installRunner, cancelRunner, uninstallRunner, type ProtonFamily, type RunnerRelease, type RunnerProgress } from "../../services/protonService";

const WINE_NOTE = "runtime_note_1" as const;
const distributions: readonly { family: ProtonFamily; name: string; author: string; icon: string; note?: keyof Catalog["ui-proton-distribution-cards"] }[] = [
  { family: "ge-proton", name: "Proton-GE Latest", author: "GloriousEggroll", icon: "/icons/ge-proton.png" },
  { family: "cachyos-proton", name: "Proton-CachyOS Latest", author: "CachyOS", icon: "/icons/cachyos.svg" },
  { family: "cachyos-wineland", name: "Proton-CachyOS Wineland Latest", author: "nanomatters", icon: "/icons/cachyos.svg",
    note: "runtime_note_2" },
  { family: "wine-staging", name: "Wine-Staging Latest", author: "Kron4ek", icon: "/icons/wine.svg", note: WINE_NOTE },
  { family: "wine-proton", name: "Wine-Proton Latest", author: "Kron4ek", icon: "/icons/wine.svg", note: WINE_NOTE },
];

export function ProtonDistributionCards({ disabled, onBusy }: {
  disabled: boolean; onBusy: (busy: boolean) => void;
}) {

  const { t } = useI18n();

  const [releases, setReleases] = useState<Partial<Record<ProtonFamily, RunnerRelease>>>({});
  const [checking, setChecking] = useState<Partial<Record<ProtonFamily, boolean>>>({});
  const [messages, setMessages] = useState<Partial<Record<ProtonFamily, string>>>({});
  const [active, setActive] = useState<ProtonFamily | null>(null);
  const [progressFamily, setProgressFamily] = useState<ProtonFamily | null>(null);
  const [progress, setProgress] = useState<RunnerProgress | null>(null);
  const [removal, setRemoval] = useState<{ family: ProtonFamily; name: string; version: string } | null>(null);
  const [removing, setRemoving] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const lock = useRef(false), mounted = useRef(true);
  const checks = useRef(new Set<ProtonFamily>());
  const refresh = async (f: ProtonFamily) => {
    if (checks.current.has(f)) return;
    checks.current.add(f);
    setChecking(old => ({ ...old, [f]: true }));
    setMessages(old => ({ ...old, [f]: "" }));
    try { const release = await checkRunner(f); if (!release?.asset) throw new Error(t("runners", "releaseError")); if (mounted.current) setReleases(old => ({ ...old, [f]: release })); }
    catch (error) { if (mounted.current) { setReleases(old => ({ ...old, [f]: undefined })); setMessages(old => ({ ...old, [f]: error instanceof Error ? error.message : String(error) })); } }
    finally { checks.current.delete(f); if (mounted.current) setChecking(old => ({ ...old, [f]: false })); }
  };
  useEffect(() => {
    mounted.current = true;
    distributions.forEach(({ family: f }) => void refresh(f));
    return () => { mounted.current = false; };
  }, []);
  const download = async (f: ProtonFamily) => {
    if (lock.current || disabled || checks.current.has(f) || !releases[f]?.needs_update) return;
    lock.current = true; setActive(f); setProgressFamily(f); setProgress(null); setCancelling(false); onBusy(true);
    setMessages(old => ({ ...old, [f]: "" }));
    try {
      await installRunner(f, p => { if (mounted.current && p.phase !== "complete") setProgress(p); });
      window.dispatchEvent(new Event("protonRunnersChanged"));
      await refresh(f);
      if (mounted.current) {
        setProgress(p => ({phase:"complete",family:f,downloaded:p?.downloaded??0,total:p?.total??0,speed_bytes_per_sec:0}));
        setMessages(old => ({ ...old, [f]: t("runners", "installed") }));
      }
    } catch (error) { if (mounted.current) { setProgress(null); setMessages(old => ({ ...old, [f]: error instanceof Error ? error.message : String(error) })); } }
    finally { lock.current = false; if (mounted.current) { setActive(null); setCancelling(false); } onBusy(false); }
  };
  const cancel = async () => {
    if (cancelling || !active) return;
    setCancelling(true);
    try { await cancelRunner(); }
    catch (error) { setMessages(old => ({ ...old, [active]: error instanceof Error ? error.message : String(error) })); setCancelling(false); }
  };
  const closeRemoval = () => { if (!removing) { setRemoval(null); onBusy(false); } };
  const uninstall = async () => {
    if (!removal || lock.current) return;
    const { family: f, version } = removal;
    lock.current = true; setRemoving(true);
    setMessages(old => ({ ...old, [f]: t("runners", "uninstalling") }));
    try {
      await uninstallRunner(f, version);
      setReleases(old => ({ ...old, [f]: old[f] ? { ...old[f]!, current: null, installed_path: null, needs_update: true } : undefined }));
      window.dispatchEvent(new Event("protonRunnersChanged"));
      setMessages(old => ({ ...old, [f]: t("runners", "uninstalled") }));
    } catch (error) { setMessages(old => ({ ...old, [f]: error instanceof Error ? error.message : String(error) })); }
    finally { lock.current = false; setRemoving(false); setRemoval(null); onBusy(false); }
  };
  return <><div className="proton-download-grid">
    {distributions.map(d=>{
      const release=releases[d.family],downloading=active===d.family;
      const stateLabel=downloading?(t("ui-proton-distribution-cards", "installing")):checking[d.family]?(t("ui-proton-distribution-cards", "checking")):!release?(t("ui-proton-distribution-cards", "not_checked")):!release.current?t("runners","notInstalled"):release.needs_update?(t("ui-proton-distribution-cards", "update_available")):t("updater","updated");
      const currentProgress=progress||{phase:"connecting",downloaded:0,total:null,speed_bytes_per_sec:0};
      return <section key={d.family} className="proton-runtime-card mod-workspace" aria-label={d.name} aria-busy={downloading}>
        <header className="proton-runtime-heading"><span className="proton-runtime-icon"><img src={d.icon} alt=""/></span><div><h4>{d.name.replace(" Latest","")}</h4><span>{d.author}</span></div><span className="mod-status-badge" data-ready={Boolean(release?.current&&!release.needs_update)} role="status">{stateLabel}</span></header>
        {d.note&&<p className="proton-runtime-note">{t("ui-proton-distribution-cards", d.note)}</p>}
        <dl className="proton-runtime-versions"><div><dt>{t("ui-proton-distribution-cards", "installed")}</dt><dd>{release?.current||(t("ui-proton-distribution-cards", "none"))}</dd></div><div><dt>{t("ui-proton-distribution-cards", "available")}</dt><dd>{release?.latest||"—"}{release&&release.asset.size>0&&<small>{formatTransferBytes(release.asset.size)}</small>}</dd></div></dl>
        <div className="proton-runtime-actions">
          {downloading?<button type="button" className="mod-action-secondary" disabled={cancelling||progress?.phase==="complete"} onClick={()=>void cancel()}><MenuIcon name="stop"/>{t("runners",cancelling?"cancelling":"cancel")}</button>:<>
            {(!release?.current||release.needs_update)&&<button type="button" className="mod-action-primary" disabled={disabled||Boolean(active)||checking[d.family]||!release?.needs_update} onClick={()=>void download(d.family)}><MenuIcon name="download"/>{release?.current?(t("ui-proton-distribution-cards", "update")):(t("ui-proton-distribution-cards", "install"))}</button>}
            <button type="button" className="mod-action-secondary" disabled={disabled||Boolean(active)||checking[d.family]} onClick={()=>void refresh(d.family)}><MenuIcon name="repair"/>{checking[d.family]?(t("ui-proton-distribution-cards", "checking")):(t("ui-proton-distribution-cards", "check_for_updates"))}</button>
          </>}
        </div>
        {progressFamily===d.family&&(downloading||progress?.phase==="complete")&&<ComponentDownloadProgress progress={currentProgress} onDismiss={()=>{setProgress(null);setMessages(old=>old[d.family]===t("runners","installed")?{...old,[d.family]:""}:old)}} label={progress?.phase==="complete"?(t("ui-proton-distribution-cards", "installation_complete")):cancelling?t("runners","cancelling"):progress?t("runners",progress.phase):(t("ui-proton-distribution-cards", "preparing_download"))}/>}
        {release?.installed_path&&<div className="mod-destination"><MenuIcon name="folder"/><div><span>{t("ui-proton-distribution-cards", "install_location")}</span><button type="button" title={release.installed_path} disabled={downloading} onClick={()=>void invoke("open_folder",{path:release.installed_path}).catch(error=>notifyError(d.name,String(error)))}>{release.installed_path}</button></div></div>}
        {release?.current&&<div className="mod-setting-row mod-destructive-row"><div className="mod-row-copy"><strong>{t("ui-proton-distribution-cards", "remove_this_version")}</strong><small>{t("ui-proton-distribution-cards", "games_using_this_version_will_need_another_selection")}</small></div><button type="button" className="mod-action-quiet danger" disabled={disabled||Boolean(active)||checking[d.family]} onClick={()=>{setRemoval({family:d.family,name:d.name,version:release.current!});onBusy(true)}}><MenuIcon name="trash"/>{t("ui-proton-distribution-cards", "remove")}</button></div>}
        {messages[d.family]&&<p className="proton-runtime-message" role="status">{messages[d.family]}</p>}
      </section>;
    })}
  </div>
    {removal && <ConfirmModal title={t("runners", "uninstall")} message={t("runners", "uninstallConfirm").replace("{name}", removal.name).replace("{version}", removal.version)} confirmText={t("runners", "uninstall")} cancelText={t("confirm", "cancel")} isDanger busy={removing} onConfirm={() => void uninstall()} onCancel={closeRemoval}/>}
  </>;
}

import { useEffect, useRef, useState } from "react";
import { ConfirmModal } from "../ConfirmModal";
import { MenuIcon } from "../MenuIcon";
import { useI18n } from "../../i18n/I18nContext";
import { checkRunner, installRunner, cancelRunner, uninstallRunner, type ProtonFamily, type RunnerRelease, type RunnerProgress } from "../../services/protonService";

const distributions = [
  { family: "ge-proton", name: "Proton-GE Latest", author: "GloriousEggroll", icon: "/icons/ge-proton.png" },
  { family: "cachyos-proton", name: "Proton-CachyOS Latest", author: "CachyOS", icon: "/icons/cachyos.svg" },
] as const;

export function ProtonDistributionCards({ disabled, onBusy }: {
  disabled: boolean; onBusy: (busy: boolean) => void;
}) {
  const { t } = useI18n();
  const [releases, setReleases] = useState<Partial<Record<ProtonFamily, RunnerRelease>>>({});
  const [checking, setChecking] = useState<Partial<Record<ProtonFamily, boolean>>>({});
  const [messages, setMessages] = useState<Partial<Record<ProtonFamily, string>>>({});
  const [active, setActive] = useState<ProtonFamily | null>(null);
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
    lock.current = true; setActive(f); setProgress(null); setCancelling(false); onBusy(true);
    setMessages(old => ({ ...old, [f]: "" }));
    try {
      await installRunner(f, p => { if (mounted.current) setProgress(p); });
      window.dispatchEvent(new Event("protonRunnersChanged"));
      await refresh(f);
      if (mounted.current) setMessages(old => ({ ...old, [f]: t("runners", "installed") }));
    } catch (error) { if (mounted.current) setMessages(old => ({ ...old, [f]: error instanceof Error ? error.message : String(error) })); }
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
  return <><div className="preference-choice-list proton-distribution-list">
    {distributions.map(d => {
      const release = releases[d.family], downloading = active === d.family;
      const phase = progress?.phase ?? "downloading";
      const percent = progress?.total ? Math.min(100, progress.downloaded / progress.total * 100) : 0;
      const status = downloading ? `${t("runners", cancelling ? "cancelling" : phase)}${!cancelling && phase === "downloading" ? `… ${percent.toFixed(0)}% (${((progress?.speed_bytes_per_sec ?? 0) / 1024 / 1024).toFixed(1)} MB/s)` : ""}` : checking[d.family] ? t("updater", "checking") : release ? `${release.latest} · ${release.needs_update ? (release.current ? t("updater", "update") : t("runners", "notInstalled")) : t("updater", "updated")}` : d.author;
      return <div key={d.family} className={`proton-distribution-card ${downloading ? "is-downloading" : ""}`}>
        {downloading && <span className="proton-card-progress" style={{ transform: `scaleX(${phase === "downloading" ? percent / 100 : 1})` }}/>}
        <div className="proton-distribution-info">
          <img src={d.icon} alt=""/><span><strong>{d.name}</strong><small role="status">{status}</small></span>
        </div>
        <div className="proton-card-actions">
          {downloading ? <button type="button" className="proton-card-cancel" disabled={cancelling || phase === "complete"} aria-label={`${t("runners", "cancel")}: ${d.name}`} title={t("runners", "cancel")} onClick={() => void cancel()}><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" aria-hidden="true"><path d="m7 7 10 10M17 7 7 17"/></svg></button> : <>
            {release?.current && <button type="button" disabled={disabled || !!active || checking[d.family]} aria-label={`${t("runners", "uninstall")}: ${d.name}`} title={t("runners", "uninstall")} onClick={() => { setRemoval({ family: d.family, name: d.name, version: release.current! }); onBusy(true); }}><MenuIcon name="trash"/></button>}
            {(!release?.current || release.needs_update) && <button type="button" disabled={disabled || !!active || checking[d.family] || !release?.needs_update} aria-label={`${t("runners", "download")}: ${d.name}`} title={release && !release.needs_update ? t("updater", "updated") : t("runners", "download")} onClick={() => void download(d.family)}><MenuIcon name="download"/></button>}
            <button type="button" className={checking[d.family] ? "is-checking" : ""} disabled={disabled || !!active || checking[d.family]} aria-label={`${t("runners", "check")}: ${d.name}`} title={t("runners", "check")} onClick={() => void refresh(d.family)}><MenuIcon name="repair"/></button>
          </>}
        </div>
        {messages[d.family] && <p className="proton-card-message" role="status">{messages[d.family]}</p>}
      </div>;
    })}
  </div>
    {removal && <ConfirmModal title={t("runners", "uninstall")} message={t("runners", "uninstallConfirm").replace("{name}", removal.name).replace("{version}", removal.version)} confirmText={t("runners", "uninstall")} cancelText={t("confirm", "cancel")} isDanger busy={removing} onConfirm={() => void uninstall()} onCancel={closeRemoval}/>}
  </>;
}

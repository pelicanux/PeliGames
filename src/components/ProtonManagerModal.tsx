import { useEffect, useState } from "react";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { checkRunner, installRunner, cancelRunner, type ProtonFamily, type RunnerRelease, type RunnerProgress } from "../services/protonService";
const families: ProtonFamily[] = ["ge-proton", "cachyos-proton"];
const names = { "ge-proton": "GE-Proton", "cachyos-proton": "Proton CachyOS" };
export function ProtonManagerModal({ initialFamily, onClose, onInstalled, isEmbedded = false, onBack, onBusyChange }: { initialFamily: ProtonFamily; onClose: () => void; onInstalled: (path: string) => void; isEmbedded?: boolean; onBack?: () => void; onBusyChange?: (busy: boolean) => void }) {
  const { t } = useI18n();
  const [family, setFamily] = useState(initialFamily);
  const [releases, setReleases] = useState<Partial<Record<ProtonFamily, RunnerRelease>>>({});
  const [errors, setErrors] = useState<Partial<Record<ProtonFamily, string>>>({});
  const [checking, setChecking] = useState(true), [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<RunnerProgress | null>(null);
  const [message, setMessage] = useState("");
  const [cancelPending, setCancelPending] = useState(false);
  const refresh = async () => {
    setChecking(true); setErrors({});
    await Promise.all(families.map(async f => {
      try { const release = await checkRunner(f); setReleases(old => ({ ...old, [f]: release })); }
      catch (e) { setErrors(old => ({ ...old, [f]: e instanceof Error ? e.message : String(e) })); setReleases(old => ({ ...old, [f]: undefined })); }
    })); setChecking(false);
  };
  useEffect(() => { void refresh(); }, []);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { event.stopImmediatePropagation(); if (!busy) onClose(); } };
    window.addEventListener("keydown", escape, true); return () => window.removeEventListener("keydown", escape, true);
  }, [busy, onClose]);
  const release = releases[family];
  const percent = progress?.total ? Math.min(100, progress.downloaded / progress.total * 100) : 0;
  const download = async () => {
    setBusy(true); onBusyChange?.(true); setCancelPending(false); setProgress(null); setMessage("");
    try { const path = await installRunner(family, setProgress); onInstalled(path); setMessage(t("runners", "installed")); await refresh(); }
    catch (e) { setMessage(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); onBusyChange?.(false); setCancelPending(false); }
  };
  const cancel = async () => { setCancelPending(true); try { await cancelRunner(); } catch (e) { setMessage(e instanceof Error ? e.message : String(e)); setCancelPending(false); } };
  const phase = progress?.phase ?? "downloading";
  const content = <ModalSurface className={isEmbedded ? "proton-manager-embedded" : "modal-content proton-manager-modal"} header={isEmbedded ? undefined : <h2>{t("runners", "title")}</h2>} onDismiss={onClose} closeDisabled={busy}>
      {isEmbedded && <>
        <button type="button" className="btn btn-secondary backend-back-button" disabled={busy} onClick={onBack ?? onClose}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M19 12H5m7-7-7 7 7 7" /></svg>{t("updater", "back")}
        </button>
        <h3 className="modal-title">{t("runners", "title")}</h3>
      </>}
      <p className="proton-manager-description">{t("runners", "description")}</p>
      <div className="proton-family-tabs" role="group" aria-label={t("runners", "family")}>
        {families.map(f => <button key={f} type="button" disabled={busy} className={family === f ? "selected" : ""} aria-pressed={family === f} onClick={() => { setFamily(f); setMessage(""); setProgress(null); }}>{names[f]}</button>)}
      </div>
      {checking ? <p role="status">{t("updater", "checking")}</p> : errors[family] ? <p role="alert" className="proton-manager-error">{errors[family]}</p> : release && <>
        <dl className="proton-release-details"><dt>{t("runners", "latest")}</dt><dd>{release.latest}</dd><dt>{t("runners", "current")}</dt><dd>{release.current ?? t("runners", "notInstalled")}</dd><dt>{t("runners", "package")}</dt><dd>{release.asset.name} · {(release.asset.size / 1024 / 1024).toFixed(0)} MB</dd></dl>
        <p className="proton-manager-description">{t("runners", "versionsKept")}</p>
      </>}
      <div className="proton-download-actions">
        <button type="button" className={`primary proton-download-button ${busy ? "is-downloading" : ""}`} disabled={checking || busy || !release || !release.needs_update} onClick={() => void download()}>
          {busy && <span className="proton-download-fill" style={{ width: `${percent}%` }} />}
          <span><MenuIcon name={release && !release.needs_update ? "check" : "download"} />{busy ? `${t("runners", phase)} ${phase === "downloading" ? `${percent.toFixed(0)}% · ${((progress?.speed_bytes_per_sec ?? 0) / 1024 / 1024).toFixed(1)} MB/s` : ""}` : release && !release.needs_update ? t("updater", "updated") : release?.current ? t("updater", "update") : t("runners", "download")}</span>
        </button>
        {busy && <button type="button" disabled={cancelPending} onClick={() => void cancel()}>{t("runners", cancelPending ? "cancelling" : "cancel")}</button>}
        {!busy && <button type="button" disabled={checking} onClick={() => void refresh()}><MenuIcon name="repair" />{t("runners", "check")}</button>}
      </div>
      {busy && <p className="proton-manager-description" role="status">{t("runners", "keepOpen")}</p>}
      {message && <p className="proton-manager-status" role="status">{message}</p>}
      <p className="proton-manager-location">{t("runners", "location")}<code>~/.config/peligames/runners/proton</code></p>
    </ModalSurface>;
  return isEmbedded ? content : <div className="modal-overlay" onClick={event => { if (event.target === event.currentTarget && !busy) onClose(); }}>{content}</div>;
}

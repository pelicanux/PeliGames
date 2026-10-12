import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { notifyError } from "../services/notifications";

interface AppImageStatus { appimage: boolean; installed: boolean; target: string }

/** First start: a short presentation and, for an AppImage not installed yet, the install button. */
export function WelcomeModal({ onStart }: { onStart: () => void }) {
  const { t } = useI18n();
  const [status, setStatus] = useState<AppImageStatus | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if ("__PELI_UI_PREVIEW__" in window) return;
    void invoke<AppImageStatus>("appimage_install_status").then(setStatus).catch(() => setStatus(null));
  }, []);
  const offerInstall = Boolean(status?.appimage && !status.installed);
  // The installed copy opens and this one closes; the welcome screen is not shown again.
  const install = async () => {
    if (installing) return;
    setInstalling(true); setError("");
    try { localStorage.setItem("peligames_welcome_completed", "1"); } catch { /* The installed copy shows the welcome again. */ }
    try { await invoke("install_peligames_appimage"); }
    catch (e) { setError(String(e)); notifyError(t("welcome", "installTitle"), e); setInstalling(false); }
  };
  const features: [Parameters<typeof MenuIcon>[0]["name"], string][] = [
    ["gamepad", t("welcome", "library")],
    ["download", t("welcome", "install")],
    ["puzzle", t("welcome", "mods")],
    ["cube", t("welcome", "launchers")],
  ];
  return <div className="modal-overlay">
    <ModalSurface hideClose className="modal-content dialog-glass welcome-modal" role="dialog" aria-modal="true" aria-labelledby="launcher-welcome-title">
      <img className="welcome-logo" src="/peligames.svg" alt="" />
      <h2 id="launcher-welcome-title">{t("welcome", "title")}</h2>
      <p className="welcome-subtitle">{t("welcome", "subtitle")}</p>
      <ul className="welcome-features">
        {features.map(([icon, text]) => <li key={icon}><MenuIcon name={icon} /><span>{text}</span></li>)}
      </ul>
      {offerInstall && <div className="welcome-install">
        <strong>{t("welcome", "installTitle")}</strong>
        <p>{t("welcome", "installHint")}</p>
        <code>{status?.target}</code>
      </div>}
      {error && <p role="alert" className="preferences-error">{error}</p>}
      {offerInstall ? <div className="welcome-actions">
        <button type="button" disabled={installing} onClick={onStart}>{t("welcome", "later")}</button>
        <button type="button" className="primary" autoFocus disabled={installing} onClick={() => void install()}><MenuIcon name="download" />{installing ? t("welcome", "installing") : t("welcome", "installButton")}</button>
      </div> : <button type="button" className="primary" autoFocus onClick={onStart}>{t("welcome", "start")}</button>}
    </ModalSurface>
  </div>;
}

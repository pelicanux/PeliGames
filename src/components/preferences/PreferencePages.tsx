import { useEffect, useState, type RefObject } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AppConfig } from "../SetupWizard";
import { MenuIcon } from "../MenuIcon";
import { useI18n } from "../../i18n/I18nContext";
import { useAccentTheme } from "../../theme/ThemeProvider";
import { ACCENT_THEMES } from "../../theme/accentTheme";
import { ProtonDistributionCards } from "./ProtonDistributionCards";
import { LaunchEffectIcon } from "./LaunchEffectIcon";
import { ModBackupPreferences } from "./ModBackupPreferences";
import { usePreferencesText } from "./labels";
import { notifyError } from "../../services/notifications";

type ConfigProps = { config: AppConfig; disabled: boolean; save: (patch: Partial<AppConfig>) => void };
export function ThemePreferences({ onApplied }: { onApplied: () => void }) {
  const text = usePreferencesText(), { t } = useI18n();
  const { accent, appearance, setAccent, setAppearance, glassEffects, setGlassEffects, resetGlassEffects, launchEffect, setLaunchEffect } = useAccentTheme();
  const launchEffects = [
    { id: "smooth", name: text.smoothEffect, hint: text.smoothHint },
    { id: "shrink", name: text.shrinkEffect, hint: text.shrinkHint },
    { id: "shatter", name: text.shatterEffect, hint: text.shatterHint },
    { id: "particles", name: text.particlesEffect, hint: text.particlesHint },
    { id: "freeze", name: text.freezeEffect, hint: text.freezeHint },
    { id: "aquarium", name: text.aquariumEffect, hint: text.aquariumHint },
    { id: "penguin", name: text.penguinEffect, hint: text.penguinHint },
  ] as const;
  return <>
    <h2>{text.themes}</h2><p>{text.themesHint}</p>
    <div data-preference-id="themes" className="preference-theme-options">
      {(["dark", "light"] as const).map(theme => <button type="button" key={theme} className={`preference-choice ${appearance === theme ? "selected" : ""}`} aria-pressed={appearance === theme} onClick={() => { setAppearance(theme); onApplied(); }}>
        <svg width="25" height="25" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">{theme === "dark" ? <path d="M20.9 13a9 9 0 0 1-9.9-9.9A9 9 0 1 0 20.9 13Z"/> : <><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l2 2m10 10 2 2M5 19l2-2m10-10 2-2"/></>}</svg>
        <span><strong>{t("theme", theme)}</strong><small>{theme === "dark" ? text.darkHint : text.lightHint}</small></span><i className="preference-radio"/>
      </button>)}
    </div>
    <h3>{text.accent}</h3><div data-preference-id="accent" className="preference-swatches" role="group" aria-label={text.accent}>
      {ACCENT_THEMES.map(theme => <button type="button" key={theme.id} aria-label={t("theme", theme.id)} title={t("theme", theme.id)} aria-pressed={accent === theme.id} style={{ background: theme.color }} onClick={() => { setAccent(theme.id); onApplied(); }}>{accent === theme.id && <MenuIcon name="check"/>}</button>)}
    </div>
    <section className="preference-glass-settings">
      <h3>{text.glass}</h3><p>{text.glassHint}</p>
      {(["windows", "backdrop"] as const).map(key => <div data-preference-id={`blur-${key}`} className="preference-glass-slider" key={key}>
        <div><label htmlFor={`glass-effect-${key}`}>{key === "windows" ? text.windowsEffect : text.backdropEffect}</label><output htmlFor={`glass-effect-${key}`}>{glassEffects[key]}%</output></div>
        <input id={`glass-effect-${key}`} type="range" min="0" max="100" step="1" value={glassEffects[key]} onChange={event => { setGlassEffects({ ...glassEffects, [key]: Number(event.target.value) }); onApplied(); }} />
        <small><span>{text.transparent}</span><span>{text.opaque}</span></small>
      </div>)}
      <button type="button" data-preference-id="reset-glass" className="preference-outline" onClick={() => { resetGlassEffects(); onApplied(); }}><MenuIcon name="repair"/>{text.resetGlass}</button>
    </section>
    <section data-preference-id="launch-effect" className="preference-section">
      <h3>{text.launchEffect}</h3><p>{text.launchEffectHint}</p>
      <div role="group" aria-label={text.launchEffect}>
        <div className="preference-launch-effects">
          {launchEffects.map(effect => <button type="button" key={effect.id} className={`preference-choice preference-launch-effect ${launchEffect === effect.id ? "selected" : ""}`} aria-pressed={launchEffect === effect.id} onClick={() => { setLaunchEffect(effect.id); onApplied(); }}>
            <span className="preference-launch-effect-heading"><span className="preference-launch-effect-icon"><LaunchEffectIcon effect={effect.id} /></span><strong>{effect.name}</strong><i className="preference-radio" aria-hidden="true" /></span>
            <small>{effect.hint}</small>
          </button>)}
        </div>
        <button type="button" className={`preference-choice preference-launch-disable ${launchEffect === "disabled" ? "selected" : ""}`} aria-pressed={launchEffect === "disabled"} onClick={() => { setLaunchEffect("disabled"); onApplied(); }}>
          <span><strong>{text.noLaunchEffect}</strong><small>{text.noLaunchEffectHint}</small></span><i className="preference-radio" aria-hidden="true" />
        </button>
      </div>
    </section>
    <div className="preference-preview"><small>{text.preview}</small><div><MenuIcon name="gamepad"/><strong>{text.library}</strong><span className="preference-preview-bars"><i/><i/></span><span className="preference-preview-play"><MenuIcon name="play"/>{text.play}</span></div></div>
  </>;
}
export function ModPreferences({ disabled, onUpdate, onWizard, onClear }: { disabled: boolean; onUpdate: (architecture: "rdna3" | "rdna4") => void; onWizard: () => void; onClear: () => void }) {
  const text = usePreferencesText(), { t } = useI18n();
  return <><h2>{text.mods}</h2><h3>{text.project}</h3><p>{text.projectHint}</p>
    <div data-preference-id="backend-project"><div data-preference-id="backend-update" className="preference-choice-list">{(["rdna4", "rdna3"] as const).map(architecture => <div key={architecture} className="preference-choice preference-backend-card">
      <span><strong>{architecture === "rdna4" ? "DLSSNR-AMD" : "DLSSNR-RDNA3"}</strong><small>{architecture === "rdna4" ? text.rdna4 : text.rdna3}</small></span>
      <button type="button" className="preference-backend-update" disabled={disabled} aria-label={`${t("app", "updateBackend")}: ${architecture === "rdna4" ? "RDNA 4 / RX 9000" : "RDNA 3 / RX 7000"}`} onClick={() => onUpdate(architecture)}><MenuIcon name="download"/>{t("app", "updateBackend")}</button>
    </div>)}</div></div>
    <section className="preference-section"><h3>{text.modTools}</h3><div className="preference-tool-buttons"><button type="button" data-preference-id="setup" disabled={disabled} onClick={onWizard}><MenuIcon name="repair"/>{t("settings", "reset")}</button><button type="button" data-preference-id="clear-cache" className="danger" disabled={disabled} onClick={onClear}><MenuIcon name="trash"/>{t("gameCache", "clear")}</button></div></section>
  </>;
}
export function ProtonPreferences({ config, disabled, save, onBusy, focusRef }: ConfigProps & { onBusy: (busy: boolean) => void; focusRef: RefObject<HTMLInputElement | null> }) {
  const text = usePreferencesText();
  return <><h2>{text.protons}</h2><h3>{text.distribution}</h3><p>{text.distributionHint}</p>
    <div data-preference-id="proton-family"><ProtonDistributionCards disabled={disabled} onBusy={onBusy}/></div>
    <section className="preference-section"><h3>{text.steam}</h3><label data-preference-id="steam-scan" className="preference-switch"><input ref={focusRef} type="checkbox" checked={Boolean(config.scan_steam_protons)} disabled={disabled} onChange={event => save({ scan_steam_protons: event.target.checked })}/><span className="preference-switch-track"/><span>{text.steamScan}<small>{text.steamHint}</small></span></label></section>
  </>;
}
export function CoverPreferences({ config, disabled, save, focusRef, onError }: ConfigProps & { focusRef: RefObject<HTMLInputElement | null>; onError: (error: string) => void }) {
  const text = usePreferencesText(), { t } = useI18n();
  const [show, setShow] = useState(false), [draft, setDraft] = useState(config.steamgriddb_api_key ?? "");
  return <><h2>{text.covers}</h2><section data-preference-id="cover-api" className="preference-section first"><h3>{t("steamgrid", "title")}</h3><p>{t("steamgrid", "description")}</p><label htmlFor="steamgrid-api-key">{t("steamgrid", "keyLabel")}</label>
    <div className="steamgrid-key-controls"><input ref={focusRef} id="steamgrid-api-key" type={show ? "text" : "password"} autoComplete="off" spellCheck={false} maxLength={128} disabled={disabled} value={draft} placeholder={t("steamgrid", "placeholder")} onChange={event => { setDraft(event.target.value); save({ steamgriddb_api_key: event.target.value.trim() }); }}/><button type="button" aria-pressed={show} aria-label={show ? text.hideKey : text.showKey} onClick={() => setShow(!show)}>{t("steamgrid", show ? "hide" : "show")}</button><button type="button" disabled={disabled || !draft} onClick={() => { setDraft(""); save({ steamgriddb_api_key: "" }); }}>{t("steamgrid", "remove")}</button></div>
    <div className="steamgrid-settings-footer"><small>{t("steamgrid", "localOnly")}</small><button type="button" onClick={() => void openUrl("https://www.steamgriddb.com/profile/preferences/api").catch(error => onError(String(error)))}>{t("steamgrid", "getKey")} ↗</button></div><p>{t("steamgrid", "rescanHint")}</p>
  </section></>;
}
/** The install offered on the welcome screen, for whoever skipped it. Only for an AppImage not installed yet. */
function AppImageInstallPreferences({ onError }: { onError: (error: string) => void }) {
  const { t } = useI18n();
  const [target, setTarget] = useState(""), [installing, setInstalling] = useState(false);
  useEffect(() => {
    if ("__PELI_UI_PREVIEW__" in window) return;
    void invoke<{ appimage: boolean; installed: boolean; target: string }>("appimage_install_status").then(status => setTarget(status.appimage && !status.installed ? status.target : "")).catch(() => setTarget(""));
  }, []);
  if (!target) return null;
  // The installed copy opens and this one closes.
  const install = async () => {
    setInstalling(true);
    try { await invoke("install_peligames_appimage"); }
    catch (error) { onError(String(error)); notifyError(t("welcome", "installTitle"), error); setInstalling(false); }
  };
  return <section data-preference-id="appimage-install" className="preference-section"><h3>{t("welcome", "installTitle")}</h3><p>{t("welcome", "installHint")}</p><p><code>{target}</code></p>
    <button type="button" className="preference-outline" disabled={installing} onClick={() => void install()}><MenuIcon name="download"/>{installing ? t("welcome", "installing") : t("welcome", "installButton")}</button></section>;
}
export function ToolsPreferences({ onError, onLogs, onBusy, onRestored }: { onError: (error: string) => void; onLogs: () => void; onBusy: (busy: boolean) => void; onRestored: () => Promise<void> }) {
  const text = usePreferencesText(), { t } = useI18n();
  const [folders, setFolders] = useState<string[]>(() => { try { const value = JSON.parse(localStorage.getItem("custom_folders") ?? "[]"); return Array.isArray(value) ? value.filter(item => typeof item === "string") : []; } catch { return []; } });
  const remove = (folder: string) => { const next = folders.filter(item => item !== folder); try { localStorage.setItem("custom_folders", JSON.stringify(next)); setFolders(next); window.dispatchEvent(new Event("refreshGames")); } catch (error) { onError(String(error)); } };
  return <><h2>{text.tools}</h2><section data-preference-id="launcher-logs" className="preference-section first"><h3>{text.openLogs}</h3><p>{text.logsHint}</p><button type="button" className="preference-outline" onClick={onLogs}><MenuIcon name="logs"/>{text.openLogs}</button></section>
    <AppImageInstallPreferences onError={onError}/>
    <ModBackupPreferences onError={onError} onBusy={onBusy} onRestored={onRestored}/>
    <section data-preference-id="manual-folders" className="preference-section"><h3>{t("settings", "manualDirs")}</h3><p>{t("settings", "manualDirsDesc")}</p>{folders.length ? <div className="preference-folders">{folders.map(folder => <div key={folder}><span>{folder}</span><button type="button" aria-label={`${t("settings", "removeDir")}: ${folder}`} onClick={() => remove(folder)}><MenuIcon name="trash"/></button></div>)}</div> : <p>{text.noFolders}</p>}</section>
  </>;
}

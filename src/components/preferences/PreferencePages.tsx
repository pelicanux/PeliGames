import { useEffect, useRef, useState, type RefObject, type CSSProperties } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AppConfig } from "../SetupWizard";
import { MenuIcon } from "../MenuIcon";
import { BrandName } from "../BrandName";
import { useI18n } from "../../i18n/I18nContext";
import { useAccentTheme } from "../../theme/ThemeProvider";
import { ACCENT_THEMES } from "../../theme/accentTheme";
import { isFreeInterfaceTheme } from "../../theme/interfaceTheme";
import { ProtonDistributionCards } from "./ProtonDistributionCards";
import { LaunchEffectIcon } from "./LaunchEffectIcon";
import { usePreferencesText } from "./labels";
import { notifyError } from "../../services/notifications";
import { ConfirmModal } from "../ConfirmModal";

type ConfigProps = { config: AppConfig; disabled: boolean; save: (patch: Partial<AppConfig>) => void };
export function ThemePreferences({ onApplied, onOpenConnections }: { onApplied: () => void; onOpenConnections: () => void }) {
  const text = usePreferencesText(), { t, language } = useI18n();
  const pt = language === "pt";
  const { interfaceTheme, setInterfaceTheme, themeTransitioning, accent, appearance, setAccent, setAppearance, glassEffects, setGlassEffects, exclusiveEffects, launchEffect, setLaunchEffect } = useAccentTheme();
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
    <section data-preference-id="interface-theme" className="preference-panel">
      <h2>{pt ? "Temas da interface" : "Interface themes"}</h2>
      <p>{pt ? "Escolha uma identidade visual para todo o launcher." : "Choose a visual identity for the entire launcher."}</p>
      {!exclusiveEffects && <p className="preference-effects-locked">{text.exclusiveThemesBeforeLink}{" "}<button type="button" className="preference-connections-link" onClick={onOpenConnections}>{text.connections}</button>{" "}{text.exclusiveEffectsAfterLink}</p>}
      <div className="interface-theme-cards" role="group" aria-label={pt ? "Temas da interface" : "Interface themes"}>
        {(["standard", "hackerman"] as const).map(theme => { const locked = !exclusiveEffects && !isFreeInterfaceTheme(theme); return <button type="button" key={theme} disabled={themeTransitioning || locked} title={locked ? text.exclusiveThemesHint : undefined} className={`preference-choice interface-theme-card ${interfaceTheme === theme ? "selected" : ""}`} aria-pressed={interfaceTheme === theme} onClick={() => { setInterfaceTheme(theme); onApplied(); }}>
          <span className={`interface-theme-sample ${theme}`} aria-hidden="true">{theme === "hackerman" ? ">_ HACKERMAN" : <><img className="interface-theme-brand-logo" src="/peligames.svg" alt=""/><span className="interface-theme-brand-name"><BrandName/></span></>}</span>
          <strong>{theme === "hackerman" ? "Hackerman" : pt ? "Padrão" : "Standard"}{locked && <svg className="preference-effect-lock" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-label={text.lockedTheme}><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>}</strong>
          <small>{theme === "hackerman" ? pt ? "Preto e verde terminal, chuva digital e tela retrô." : "Black and terminal green, digital rain and a retro screen." : pt ? "Interface original com suas cores e aparência favoritas." : "Original interface with your preferred colors and appearance."}</small>
        </button>; })}
      </div>
      {interfaceTheme === "hackerman" && <p>{pt ? "Hackerman usa sua própria paleta escura. Suas cores e aparência anteriores ficam guardadas no tema Padrão." : "Hackerman uses its own dark palette. Your previous colors and appearance are preserved in Standard."}</p>}
    </section>
    <section className="preference-panel">
    <h2>{text.themes}</h2><p>{text.themesHint}</p>
    <div data-preference-id="themes" className="preference-theme-options">
      {(["dark", "light"] as const).map(theme => <button type="button" key={theme} disabled={interfaceTheme === "hackerman" || themeTransitioning} className={`preference-choice ${appearance === theme ? "selected" : ""}`} aria-pressed={appearance === theme} onClick={() => { setAppearance(theme); onApplied(); }}>
        <svg width="25" height="25" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">{theme === "dark" ? <path d="M20.9 13a9 9 0 0 1-9.9-9.9A9 9 0 1 0 20.9 13Z"/> : <><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l2 2m10 10 2 2M5 19l2-2m10-10 2-2"/></>}</svg>
        <span><strong>{t("theme", theme)}</strong><small>{theme === "dark" ? text.darkHint : text.lightHint}</small></span><i className="preference-radio"/>
      </button>)}
    </div>
    </section>
    <section className="preference-panel preference-personalization">
      <h3>{text.accentAndBlur}</h3>
      <div className="preference-accent-blur">
        <div data-preference-id="accent" className="preference-swatches" role="group" aria-label={text.accent}>
          {ACCENT_THEMES.map(theme => <button type="button" key={theme.id} disabled={interfaceTheme === "hackerman" || themeTransitioning} aria-label={t("theme", theme.id)} title={t("theme", theme.id)} aria-pressed={accent === theme.id} style={{ background: theme.color }} onClick={() => { setAccent(theme.id); onApplied(); }}>{accent === theme.id && <MenuIcon name="check"/>}</button>)}
        </div>
        <span className="preference-accent-blur-divider" aria-hidden="true" />
        <div data-preference-id="blur-windows" className="preference-glass-slider">
          <div><label htmlFor="glass-effect-windows">{text.windowsEffect}</label><output htmlFor="glass-effect-windows">{glassEffects.windows === 100 ? text.opaque : `${glassEffects.windows}%`}</output></div>
          <p id="glass-effect-hint" className="preference-blur-hint">{text.glassHint}</p>
          <input id="glass-effect-windows" type="range" min="0" max="100" step="1" value={glassEffects.windows} aria-describedby="glass-effect-hint" style={{ "--blur-progress": `${glassEffects.windows}%` } as CSSProperties} onChange={event => { setGlassEffects({ windows: Number(event.target.value), backdrop: 50 }); onApplied(); }} />
          <small><span>{text.transparent}</span><span>{text.opaque}</span></small>
        </div>
      </div>
    </section>
    <section data-preference-id="launch-effect" className="preference-section">
      <h3>{text.launchEffect}</h3><p>{text.launchEffectHint}</p>
      {!exclusiveEffects && <p className="preference-effects-locked">{text.exclusiveEffectsBeforeLink}{" "}<button type="button" className="preference-connections-link" onClick={onOpenConnections}>{text.connections}</button>{" "}{text.exclusiveEffectsAfterLink}</p>}
      <div role="group" aria-label={text.launchEffect}>
        <div className="preference-launch-effects">
          {launchEffects.map(effect => <button type="button" key={effect.id} disabled={!exclusiveEffects && effect.id !== "smooth"} title={!exclusiveEffects && effect.id !== "smooth" ? text.exclusiveEffectsHint : undefined} className={`preference-choice preference-launch-effect ${launchEffect === effect.id ? "selected" : ""}`} aria-pressed={launchEffect === effect.id} onClick={() => { setLaunchEffect(effect.id); onApplied(); }}>
            <span className="preference-launch-effect-heading"><span className="preference-launch-effect-icon"><LaunchEffectIcon effect={effect.id} /></span><strong>{effect.name}</strong>{!exclusiveEffects && effect.id !== "smooth" && <svg className="preference-effect-lock" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-label={text.lockedEffect}><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>}<i className="preference-radio" aria-hidden="true" /></span>
            <small>{effect.hint}</small>
          </button>)}
          <button type="button" className={`preference-choice preference-launch-effect ${launchEffect === "disabled" ? "selected" : ""}`} aria-pressed={launchEffect === "disabled"} onClick={() => { setLaunchEffect("disabled"); onApplied(); }}>
            <span className="preference-launch-effect-heading"><span className="preference-launch-effect-icon"><LaunchEffectIcon effect="disabled" /></span><strong>{text.noLaunchEffect}</strong><i className="preference-radio" aria-hidden="true" /></span>
            <small>{text.noLaunchEffectHint}</small>
          </button>
        </div>
      </div>
    </section>
  </>;
}
export function ModPreferences({ disabled, onUpdate, onWizard, onClear }: { disabled: boolean; onUpdate: (architecture: "rdna3" | "rdna4") => void; onWizard: () => void; onClear: () => void }) {
  const text = usePreferencesText(), { t } = useI18n();
  return <><h2>{text.mods}</h2><section className="preference-panel"><h3>{text.project}</h3><p>{text.projectHint}</p>
    <div data-preference-id="backend-project"><div data-preference-id="backend-update" className="preference-choice-list preference-backend-projects">{(["rdna4", "rdna3"] as const).map(architecture => <div key={architecture} className="preference-choice preference-backend-card">
      <span><strong>{architecture === "rdna4" ? "DLSSNR-AMD" : "DLSSNR-RDNA3"}</strong><small>{architecture === "rdna4" ? text.rdna4 : text.rdna3}</small></span>
      <button type="button" className="preference-backend-update" disabled={disabled} aria-label={`${t("app", "manageBackend")}: ${architecture === "rdna4" ? "RDNA 4 / RX 9000" : "RDNA 3 / RX 7000"}`} onClick={() => onUpdate(architecture)}><MenuIcon name="download"/>{t("app", "manageBackend")}</button>
    </div>)}</div></div>
    <div className="preference-backend-tools"><h3>{text.modTools}</h3><div className="preference-tool-buttons"><button type="button" data-preference-id="setup" disabled={disabled} onClick={onWizard}><MenuIcon name="repair"/>{t("settings", "reset")}</button><button type="button" data-preference-id="clear-cache" className="danger" disabled={disabled} onClick={onClear}><MenuIcon name="trash"/>{t("gameCache", "clear")}</button></div></div></section>
  </>;
}
export function ProtonPreferences({ config, disabled, save, onBusy, focusRef, onConfirmationChange }: ConfigProps & { onBusy: (busy: boolean) => void; focusRef: RefObject<HTMLInputElement | null>; onConfirmationChange: (open: boolean) => void }) {
  const text = usePreferencesText();
  const [confirmSteam, setConfirmSteam] = useState(false);
  const cancelSteamRef = useRef<HTMLButtonElement>(null);
  const confirmationCallback = useRef(onConfirmationChange);
  confirmationCallback.current = onConfirmationChange;
  useEffect(() => { confirmationCallback.current(confirmSteam); return () => confirmationCallback.current(false); }, [confirmSteam]);
  useEffect(() => {
    if (!confirmSteam) return;
    cancelSteamRef.current?.focus({ preventScroll:true });
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); setConfirmSteam(false); focusRef.current?.focus({ preventScroll:true }); }
      if (event.key === "Tab") {
        const controls = cancelSteamRef.current?.closest('.modal-content')?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)');
        if (!controls?.length) return;
        const first = controls[0], last = controls[controls.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    };
    window.addEventListener("keydown", escape, true);
    return () => window.removeEventListener("keydown", escape, true);
  }, [confirmSteam, focusRef]);
  return <><h2>{text.protons}</h2><section className="preference-panel"><h3>{text.distribution}</h3><p>{text.distributionHint}</p>
    <div data-preference-id="proton-family"><ProtonDistributionCards disabled={disabled} onBusy={onBusy}/></div></section>
    <section className="preference-section"><h3>{text.steam}</h3><label data-preference-id="steam-scan" className="preference-switch"><input ref={focusRef} type="checkbox" checked={Boolean(config.scan_steam_protons)} disabled={disabled} onChange={event => { if (event.target.checked) setConfirmSteam(true); else save({ scan_steam_protons:false }); }}/><span className="preference-switch-track"/><span>{text.steamScan}<small>{text.steamHint}</small></span></label></section>
    {confirmSteam && <ConfirmModal title={text.steamConfirmTitle} message={text.steamConfirmMessage} confirmText={text.steamConfirmEnable} cancelText={text.steamConfirmCancel} cancelButtonRef={cancelSteamRef} busy={disabled} onCancel={() => { setConfirmSteam(false); focusRef.current?.focus({ preventScroll:true }); }} onConfirm={() => { if (disabled) return; setConfirmSteam(false); save({ scan_steam_protons:true }); }}/>}
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
/** Update channel: stable releases only (default), or test versions too. */
export const UPDATE_CHANNEL_EVENT = "peligamesUpdateChannelChanged";
function UpdateChannelPreferences({ onError }: { onError: (error: string) => void }) {
  const text = usePreferencesText();
  const [enabled, setEnabled] = useState<boolean | null>(null);
  useEffect(() => { void invoke<boolean>("launcher_experimental_updates").then(setEnabled).catch(() => setEnabled(false)); }, []);
  const change = async (value: boolean) => {
    setEnabled(value);
    try {
      await invoke("set_launcher_experimental_updates", { enabled: value });
      window.dispatchEvent(new Event(UPDATE_CHANNEL_EVENT)); // Check again with the new channel.
    } catch (error) { setEnabled(!value); onError(String(error)); }
  };
  return <section data-preference-id="experimental-updates" className="preference-section"><h3>{text.launcherUpdates}</h3>
    <label className="preference-switch"><input type="checkbox" checked={Boolean(enabled)} disabled={enabled === null} onChange={event => void change(event.target.checked)}/><span className="preference-switch-track"/><span>{text.experimentalUpdates}<small>{text.experimentalUpdatesHint}</small></span></label>
  </section>;
}

function AppImageInstallPreferences({ onError }: { onError: (error: string) => void }) {
  const { t } = useI18n();
  const [target, setTarget] = useState(""), [installing, setInstalling] = useState(false);
  useEffect(() => {
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
export function ToolsPreferences({ onError, onLogs, onWelcome }: { onWelcome: () => void; onError: (error: string) => void; onLogs: () => void }) {
  const text = usePreferencesText(), { t, language } = useI18n();
  const [folders, setFolders] = useState<string[]>(() => { try { const value = JSON.parse(localStorage.getItem("custom_folders") ?? "[]"); return Array.isArray(value) ? value.filter(item => typeof item === "string") : []; } catch { return []; } });
  const remove = (folder: string) => { const next = folders.filter(item => item !== folder); try { localStorage.setItem("custom_folders", JSON.stringify(next)); setFolders(next); window.dispatchEvent(new Event("refreshGames")); } catch (error) { onError(String(error)); } };
  return <><h2>{text.tools}</h2>
    <section data-preference-id="welcome-tour" className="preference-section first"><h3>{language === "pt" ? "Boas-vindas ao PeliGames" : "Welcome to PeliGames"}</h3><p>{language === "pt" ? "Reveja a apresentação dos recursos do launcher e seus atalhos, quando quiser." : "Revisit the launcher's features and shortcuts whenever you like."}</p><button type="button" className="preference-outline" onClick={onWelcome}><MenuIcon name="info"/>{language === "pt" ? "Rever apresentação" : "View the tour"}</button></section>
    <section data-preference-id="launcher-logs" className="preference-section"><h3>{text.openLogs}</h3><p>{text.logsHint}</p><button type="button" className="preference-outline" onClick={onLogs}><MenuIcon name="logs"/>{text.openLogs}</button></section>
    <UpdateChannelPreferences onError={onError}/>
    <AppImageInstallPreferences onError={onError}/>
    <section data-preference-id="manual-folders" className="preference-section"><h3>{t("settings", "manualDirs")}</h3><p>{t("settings", "manualDirsDesc")}</p>{folders.length ? <div className="preference-folders">{folders.map(folder => <div key={folder}><span>{folder}</span><button type="button" aria-label={`${t("settings", "removeDir")}: ${folder}`} onClick={() => remove(folder)}><MenuIcon name="trash"/></button></div>)}</div> : <p>{text.noFolders}</p>}</section>
  </>;
}

export function ToolPreparation({ tool }: { tool: "diskAutomount" | "monitoring" }) {
  const text = usePreferencesText();
  return <><h2>{text[tool]}</h2><section className="preference-section first"><h3>{text.toolPreparation}</h3><p>{tool === "diskAutomount" ? text.diskPreparation : text.monitoringPreparation}</p></section></>;
}

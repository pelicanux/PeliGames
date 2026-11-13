import {useOpenRGB} from "../../theme/openrgbSync";
import {OpenRGBPreferences} from "./OpenRGBPreferences";
import { useEffect, useLayoutEffect, useRef, useState, type RefObject, type CSSProperties, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AppConfig } from "../../types/appConfig";
import { MenuIcon } from "../MenuIcon";
import { BrandName } from "../BrandName";
import { useI18n } from "../../i18n/I18nContext";
import { useAccentTheme } from "../../theme/ThemeProvider";
import { ACCENT_THEMES } from "../../theme/accentTheme";
import {NeonWavePreferences} from "../../theme/NeonWavePreferences";
import { isFreeInterfaceTheme } from "../../theme/interfaceTheme";
import { ProtonDistributionCards } from "./ProtonDistributionCards";
import { LaunchEffectIcon } from "./LaunchEffectIcon";
import { usePreferencesText } from "./labels";
import { notifyError } from "../../services/notifications";
import { ConfirmModal } from "../ConfirmModal";
import { MaintenancePreferences } from "./MaintenancePreferences";
import { frameRateMeterShown, setFrameRateMeter } from "../../services/frameRateMeter";

type ConfigProps = { config: AppConfig; disabled: boolean; save: (patch: Partial<AppConfig>) => void };
type InterfaceTab="themes"|"appearance"|"effects";
const interfaceTabFor=(id?:string|null):InterfaceTab=>id==="launch-effect"?"effects":["themes","accent","blur-windows","openrgb"].includes(id||"")?"appearance":"themes";
export function ThemePreferences({ onApplied, onOpenConnections, searchMode=false, focusTarget, searchControl }: { onApplied: () => void; onOpenConnections: () => void; searchMode?:boolean; focusTarget?:string|null; searchControl?:ReactNode }) {

  const text = usePreferencesText(), { t } = useI18n();

  const { interfaceTheme, setInterfaceTheme, themeTransitioning, accent, appearance, setAccent, setAppearance, glassEffects, setGlassEffects, exclusiveEffects, launchEffect, setLaunchEffect } = useAccentTheme();
  const [tab,setTab]=useState<InterfaceTab>(()=>interfaceTabFor(focusTarget));
  useLayoutEffect(()=>{if(focusTarget)setTab(interfaceTabFor(focusTarget));},[focusTarget]);
  const tabs=[{id:"themes",label:t("ui-preference-pages", "themes")},{id:"appearance",label:t("ui-preference-pages", "appearance")},{id:"effects",label:t("ui-preference-pages", "effects")}] as const;
  const rgbSync=useOpenRGB();
  const syncedPalette=rgbSync.preferences.enabled&&interfaceTheme!=="hackerman";
  const lockedPalette=interfaceTheme!=="standard";
  const currentTheme=interfaceTheme==="hackerman"?"Hackerman":interfaceTheme==="neon-wave"?"Neon Wave":t("ui-preference-pages", "standard");
  const launchEffects = [
    { id: "smooth", name: text.smoothEffect, hint: text.smoothHint },
    { id: "shrink", name: text.shrinkEffect, hint: text.shrinkHint },
    { id: "shatter", name: text.shatterEffect, hint: text.shatterHint },
    { id: "particles", name: text.particlesEffect, hint: text.particlesHint },
    { id: "freeze", name: text.freezeEffect, hint: text.freezeHint },
    { id: "aquarium", name: text.aquariumEffect, hint: text.aquariumHint },
    { id: "penguin", name: text.penguinEffect, hint: text.penguinHint },
  ] as const;
  return <div className="interface-workspace mod-workspace">
    {!searchMode&&<><header className="mod-intro"><h3>{t("ui-preference-pages", "make_the_launcher_yours")}</h3><p>{t("ui-preference-pages", "choose_a_theme_adjust_the_appearance_and_customize_effects_when_launching_games")}</p></header>
    <div className="settings-tab-toolbar"><div className="preference-mod-tabs interface-tabs" role="tablist" aria-label={t("ui-preference-pages", "interface_customization")} data-active={tab} onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const index=tabs.findIndex(item=>item.id===tab);const next=tabs[event.key==="Home"?0:event.key==="End"?2:(index+(event.key==="ArrowLeft"?-1:1)+3)%3].id;setTab(next);requestAnimationFrame(()=>document.getElementById(`interface-tab-${next}`)?.focus());}}>{tabs.map(item=><button key={item.id} type="button" role="tab" id={`interface-tab-${item.id}`} aria-controls={`interface-panel-${item.id}`} aria-selected={tab===item.id} tabIndex={tab===item.id?0:-1} onClick={()=>setTab(item.id)}>{item.label}</button>)}</div>{searchControl}</div></>}
    <div className="interface-tab-panel" hidden={!searchMode&&tab!=="themes"} role={searchMode?undefined:"tabpanel"} id="interface-panel-themes" aria-labelledby={searchMode?undefined:"interface-tab-themes"}>
    <section data-preference-id="interface-theme" className="interface-theme-gallery">
      <header className="interface-section-heading"><span className="mod-component-icon"><MenuIcon name="image"/></span><div><h3>{t("ui-preference-pages", "interface_themes")}</h3><p>{t("ui-preference-pages", "a_visual_identity_for_your_entire_library")}</p></div><span className="mod-status-badge" data-ready="true">{currentTheme}</span></header>
      {!exclusiveEffects && <p className="preference-effects-locked">{text.exclusiveThemesBeforeLink}{" "}<button type="button" className="preference-connections-link" onClick={onOpenConnections}>{text.connections}</button>{" "}{text.exclusiveEffectsAfterLink}</p>}
      <div className="interface-theme-cards" role="group" aria-label={t("ui-preference-pages", "interface_themes")}>
        {(["standard", "hackerman", "neon-wave"] as const).map(theme => { const locked = !exclusiveEffects && !isFreeInterfaceTheme(theme); return <button type="button" key={theme} disabled={themeTransitioning || locked} title={locked ? text.exclusiveThemesHint : undefined} className={`preference-choice interface-theme-card ${interfaceTheme === theme ? "selected" : ""}`} aria-pressed={interfaceTheme === theme} onClick={() => { setInterfaceTheme(theme); onApplied(); }}>
          <span className={`interface-theme-sample ${theme}`} aria-hidden="true">{theme === "hackerman" ? ">_ HACKERMAN" : theme === "neon-wave" ? <span className="neon-wave-sample-title">NEON WAVE <small>♫</small></span> : <><img className="interface-theme-brand-logo" src="/peligames.svg" alt=""/><span className="interface-theme-brand-name"><BrandName/></span></>}</span>
          <span className="interface-theme-label"><strong>{theme === "hackerman" ? "Hackerman" : theme === "neon-wave" ? "Neon Wave" : t("ui-preference-pages", "standard")}{locked && <svg className="preference-effect-lock" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-label={text.lockedTheme}><rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>}</strong><span className="interface-theme-selection" aria-hidden="true">{interfaceTheme===theme&&<MenuIcon name="check"/>}</span></span>
          <small>{theme === "neon-wave" ? t("ui-preference-pages", "neon_lava_pulsing_with_music_and_game_audio") : theme === "hackerman" ? t("ui-preference-pages", "black_and_terminal_green_digital_rain_and_a_retro_screen") : t("ui-preference-pages", "original_interface_with_your_preferred_colors_and_appearance")}</small>
        </button>; })}
      </div>
      {interfaceTheme === "hackerman" && <p>{t("ui-preference-pages", "hackerman_uses_its_own_dark_palette_your_previous_colors_and_appearance_are_preserved_in")}</p>}
    </section>
    {interfaceTheme === "neon-wave" && <NeonWavePreferences/>}
    </div>
    <div className="interface-tab-panel" hidden={!searchMode&&tab!=="appearance"} role={searchMode?undefined:"tabpanel"} id="interface-panel-appearance" aria-labelledby={searchMode?undefined:"interface-tab-appearance"}>
    {lockedPalette&&<p className="interface-palette-note"><MenuIcon name="info"/>{t("ui-preference-pages", "uses_its_own_palette_select_standard_to_choose_colors_or_light_mode", {"currenttheme": currentTheme})}</p>}
    <div className="interface-appearance-grid">
    <section className="interface-settings-card">
    <header className="interface-section-heading"><span className="mod-component-icon"><MenuIcon name="eye"/></span><div><h3>{t("ui-preference-pages", "color_mode")}</h3><p>{t("ui-preference-pages", "a_comfortable_appearance_for_your_surroundings")}</p></div></header>
    <div data-preference-id="themes" className="preference-theme-options">
      {(["dark", "light"] as const).map(theme => <button type="button" key={theme} disabled={interfaceTheme !== "standard" || themeTransitioning} className={`preference-choice ${appearance === theme ? "selected" : ""}`} aria-pressed={appearance === theme} onClick={() => { setAppearance(theme); onApplied(); }}>
        <svg width="25" height="25" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">{theme === "dark" ? <path d="M20.9 13a9 9 0 0 1-9.9-9.9A9 9 0 1 0 20.9 13Z"/> : <><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l2 2m10 10 2 2M5 19l2-2m10-10 2-2"/></>}</svg>
        <span><strong>{t("theme", theme)}</strong><small>{theme === "dark" ? text.darkHint : text.lightHint}</small></span><i className="preference-radio"/>
      </button>)}
    </div>
    </section>
    <section className="interface-settings-card preference-personalization">
      <header className="interface-section-heading"><span className="mod-component-icon"><MenuIcon name="sliders"/></span><div><h3>{text.accentAndBlur}</h3><p>{t("ui-preference-pages", "adjust_the_accent_color_and_panel_finish")}</p></div></header>
      <div className="preference-accent-blur">
        <div data-preference-id="accent" className="preference-swatches" role="group" aria-label={text.accent}>
          {ACCENT_THEMES.map(theme => <button type="button" key={theme.id} disabled={interfaceTheme !== "standard" || themeTransitioning || syncedPalette} aria-label={t("theme", theme.id)} title={t("theme", theme.id)} aria-pressed={!syncedPalette && accent === theme.id} style={{ background: theme.color }} onClick={() => { setAccent(theme.id); onApplied(); }}>{!syncedPalette && accent === theme.id && <MenuIcon name="check"/>}</button>)}
        </div>
        {syncedPalette&&<small>{t("ui-preference-pages", "color_controlled_by_openrgb_disable_sync_below_to_choose_a_color_manually")}</small>}
        <span className="preference-accent-blur-divider" aria-hidden="true" />
        <div data-preference-id="blur-windows" className="preference-glass-slider">
          <div><label htmlFor="glass-effect-windows">{text.windowsEffect}</label><output htmlFor="glass-effect-windows">{glassEffects.windows === 100 ? text.opaque : `${glassEffects.windows}%`}</output></div>
          <p id="glass-effect-hint" className="preference-blur-hint">{text.glassHint}</p>
          <input id="glass-effect-windows" type="range" min="0" max="100" step="1" value={glassEffects.windows} aria-describedby="glass-effect-hint" style={{ "--blur-progress": `${glassEffects.windows}%` } as CSSProperties} onChange={event => { setGlassEffects({ windows: Number(event.target.value), backdrop: 50 }); onApplied(); }} />
          <small><span>{text.transparent}</span><span>{text.opaque}</span></small>
        </div>
      </div>
    </section>
    </div><OpenRGBPreferences/></div>
    <div className="interface-tab-panel" hidden={!searchMode&&tab!=="effects"} role={searchMode?undefined:"tabpanel"} id="interface-panel-effects" aria-labelledby={searchMode?undefined:"interface-tab-effects"}>
    <section data-preference-id="launch-effect" className="interface-effects-gallery">
      <header className="interface-section-heading"><span className="mod-component-icon"><MenuIcon name="bolt"/></span><div><h3>{text.launchEffect}</h3><p>{text.launchEffectHint}</p></div></header>
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
  </div></div>;
}
export function ProtonPreferences({ config, disabled, save, onBusy, focusRef, onConfirmationChange }: ConfigProps & { onBusy: (busy: boolean) => void; focusRef: RefObject<HTMLInputElement | null>; onConfirmationChange: (open: boolean) => void }) {
  const {t: tx} = useI18n();

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
  return <div className="proton-workspace mod-workspace"><header className="mod-intro"><h3>{text.distribution}</h3><p>{tx("ui-preference-pages", "install_your_preferred_version_then_select_it_in_each_game_s_options")}</p></header>
    <div data-preference-id="proton-family"><ProtonDistributionCards disabled={disabled} onBusy={onBusy}/></div>
    <section className="proton-steam-options"><div><h3>{text.steam}</h3><p>{tx("ui-preference-pages", "include_versions_already_installed_by_steam_among_your_game_s_options")}</p></div><label data-preference-id="steam-scan" className="preference-switch"><input ref={focusRef} type="checkbox" checked={Boolean(config.scan_steam_protons)} disabled={disabled} onChange={event => { if (event.target.checked) setConfirmSteam(true); else save({ scan_steam_protons:false }); }}/><span className="preference-switch-track"/><span>{text.steamScan}<small>{text.steamHint}</small></span></label></section>
    {confirmSteam && <ConfirmModal title={text.steamConfirmTitle} message={text.steamConfirmMessage} confirmText={text.steamConfirmEnable} cancelText={text.steamConfirmCancel} cancelButtonRef={cancelSteamRef} busy={disabled} onCancel={() => { setConfirmSteam(false); focusRef.current?.focus({ preventScroll:true }); }} onConfirm={() => { if (disabled) return; setConfirmSteam(false); save({ scan_steam_protons:true }); }}/>}
  </div>;
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
  const [appimage, setAppimage] = useState(false);
  useEffect(() => {
    void invoke<{ appimage: boolean; installed: boolean; target: string }>("appimage_install_status").then(status => { setAppimage(status.appimage || status.installed); setTarget(status.appimage && !status.installed ? status.target : ""); }).catch(() => setTarget(""));
  }, []);
  if (!target) return appimage ? <MaintenancePreferences uninstall/> : null;
  // The installed copy opens and this one closes.
  const install = async () => {
    setInstalling(true);
    try { await invoke("install_peligames_appimage"); }
    catch (error) { onError(String(error)); notifyError(t("welcome", "installTitle"), error); setInstalling(false); }
  };
  return <section data-preference-id="appimage-install" className="preference-section"><h3>{t("welcome", "installTitle")}</h3><p>{t("welcome", "installHint")}</p><p><code>{target}</code></p>
    <button type="button" className="preference-outline" disabled={installing} onClick={() => void install()}><MenuIcon name="download"/>{installing ? t("welcome", "installing") : t("welcome", "installButton")}</button><MaintenancePreferences uninstall/></section>;
}
function DeveloperPreferences() {
  const text = usePreferencesText();
  const [meter, setMeter] = useState(frameRateMeterShown);
  const change = (shown: boolean) => { setFrameRateMeter(shown); setMeter(shown); };
  return <section data-preference-id="developer-mode" className="preference-section"><details className="maintenance-details"><summary>{text.developerMode}</summary><p>{text.developerModeHint}</p>
    <label className="preference-switch"><input type="checkbox" checked={meter} onChange={event => change(event.target.checked)}/><span className="preference-switch-track"/><span>{text.frameRateMeter}<small>{text.frameRateMeterHint}</small></span></label>
  </details></section>;
}
export function ToolsPreferences({ onError, onLogs, onWelcome }: { onWelcome: () => void; onError: (error: string) => void; onLogs: () => void }) {

  const text = usePreferencesText(), { t } = useI18n();
  const [folders, setFolders] = useState<string[]>(() => { try { const value = JSON.parse(localStorage.getItem("custom_folders") ?? "[]"); return Array.isArray(value) ? value.filter(item => typeof item === "string") : []; } catch { return []; } });
  const remove = (folder: string) => { const next = folders.filter(item => item !== folder); try { localStorage.setItem("custom_folders", JSON.stringify(next)); setFolders(next); window.dispatchEvent(new Event("refreshGames")); } catch (error) { onError(String(error)); } };
  return <><h2>{text.tools}</h2>
    <section data-preference-id="welcome-tour" className="preference-section first"><h3>{t("ui-preference-pages", "welcome_to_peligames")}</h3><p>{t("ui-preference-pages", "revisit_the_launcher_s_features_and_shortcuts_whenever_you_like")}</p><button type="button" className="preference-outline" onClick={onWelcome}><MenuIcon name="info"/>{t("ui-preference-pages", "view_the_tour")}</button></section>
    <section data-preference-id="launcher-logs" className="preference-section"><h3>{text.openLogs}</h3><p>{text.logsHint}</p><button type="button" className="preference-outline" onClick={onLogs}><MenuIcon name="logs"/>{text.openLogs}</button></section>
    <DeveloperPreferences/>
    <UpdateChannelPreferences onError={onError}/>
    <MaintenancePreferences/>
    <AppImageInstallPreferences onError={onError}/>
    <section data-preference-id="manual-folders" className="preference-section"><h3>{t("settings", "manualDirs")}</h3><p>{t("settings", "manualDirsDesc")}</p>{folders.length ? <div className="preference-folders">{folders.map(folder => <div key={folder}><span>{folder}</span><button type="button" aria-label={`${t("settings", "removeDir")}: ${folder}`} onClick={() => remove(folder)}><MenuIcon name="trash"/></button></div>)}</div> : <p>{text.noFolders}</p>}</section>
  </>;
}

export function ToolPreparation({ tool }: { tool: "diskAutomount" | "monitoring" }) {
  const text = usePreferencesText();
  return <><h2>{text[tool]}</h2><section className="preference-section first"><h3>{text.toolPreparation}</h3><p>{tool === "diskAutomount" ? text.diskPreparation : text.monitoringPreparation}</p></section></>;
}

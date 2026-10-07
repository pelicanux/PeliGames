import { useState, type RefObject } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "../SetupWizard";
import type { ProtonFamily } from "../../services/protonService";
import { MenuIcon } from "../MenuIcon";
import { useI18n } from "../../i18n/I18nContext";
import { useAccentTheme } from "../../theme/ThemeProvider";
import { ACCENT_THEMES } from "../../theme/accentTheme";
import { usePreferencesText } from "./labels";

type ConfigProps = { config: AppConfig; disabled: boolean; save: (patch: Partial<AppConfig>) => void };
export function ThemePreferences({ onApplied }: { onApplied: () => void }) {
  const text = usePreferencesText(), { t } = useI18n();
  const { accent, appearance, setAccent, setAppearance } = useAccentTheme();
  return <>
    <h2>{text.themes}</h2><p>{text.themesHint}</p>
    <div className="preference-theme-options">
      {(["dark", "light"] as const).map(theme => <button type="button" key={theme} className={`preference-choice ${appearance === theme ? "selected" : ""}`} aria-pressed={appearance === theme} onClick={() => { setAppearance(theme); onApplied(); }}>
        <svg width="25" height="25" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">{theme === "dark" ? <path d="M20.9 13a9 9 0 0 1-9.9-9.9A9 9 0 1 0 20.9 13Z"/> : <><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l2 2m10 10 2 2M5 19l2-2m10-10 2-2"/></>}</svg>
        <span><strong>{t("theme", theme)}</strong><small>{theme === "dark" ? text.darkHint : text.lightHint}</small></span><i className="preference-radio"/>
      </button>)}
    </div>
    <h3>{text.accent}</h3><div className="preference-swatches" role="group" aria-label={text.accent}>
      {ACCENT_THEMES.map(theme => <button type="button" key={theme.id} aria-label={t("theme", theme.id)} title={t("theme", theme.id)} aria-pressed={accent === theme.id} style={{ background: theme.color }} onClick={() => { setAccent(theme.id); onApplied(); }}>{accent === theme.id && <MenuIcon name="check"/>}</button>)}
    </div>
    <div className="preference-preview"><small>{text.preview}</small><div><MenuIcon name="gamepad"/><strong>{text.library}</strong><span className="preference-preview-bars"><i/><i/></span><span className="preference-preview-play"><MenuIcon name="play"/>{text.play}</span></div></div>
  </>;
}
export function ModPreferences({ config, disabled, save, onUpdate, onWizard, onClear }: ConfigProps & { onUpdate: () => void; onWizard: () => void; onClear: () => void }) {
  const text = usePreferencesText(), { t } = useI18n();
  return <><h2>{text.mods}</h2><h3>{text.project}</h3><p>{text.projectHint}</p>
    <div className="preference-choice-list">{(["AMDNR", "OptiScaler"] as const).map(backend => <button type="button" key={backend} disabled={disabled} aria-pressed={config.backend === backend} className={`preference-choice ${config.backend === backend ? "selected" : ""}`} onClick={() => save({ backend })}>
      <i className="preference-radio"/><span><strong>{backend === "AMDNR" ? "DLSSNR-AMD" : "DLSSNR-RDNA3"}</strong><small>{backend === "AMDNR" ? text.rdna4 : text.rdna3}</small></span><em>{backend === "AMDNR" ? "RDNA 4 / RX 9000" : "RDNA 3 / RX 7000"}</em>
    </button>)}</div>
    <section className="preference-section preference-update-row"><div><h3>{text.backendUpdate}</h3><p>{text.backendHint}</p></div><button type="button" className="preference-outline" disabled={disabled} onClick={onUpdate}><MenuIcon name="download"/>{t("app", "updateBackend")}</button></section>
    <section className="preference-section"><h3>{text.modTools}</h3><div className="preference-tool-buttons"><button type="button" disabled={disabled} onClick={onWizard}><MenuIcon name="repair"/>{t("settings", "reset")}</button><button type="button" className="danger" disabled={disabled} onClick={onClear}><MenuIcon name="trash"/>{t("gameCache", "clear")}</button></div></section>
  </>;
}
export function ProtonPreferences({ config, disabled, save, family, onFamily, onUpdate, focusRef }: ConfigProps & { family: ProtonFamily; onFamily: (family: ProtonFamily) => void; onUpdate: () => void; focusRef: RefObject<HTMLInputElement | null> }) {
  const text = usePreferencesText();
  return <><h2>{text.protons}</h2><h3>{text.distribution}</h3><p>{text.distributionHint}</p>
    <div className="preference-choice-list">{(["ge-proton", "cachyos-proton"] as const).map(option => <button type="button" key={option} disabled={disabled} aria-pressed={family === option} className={`preference-choice ${family === option ? "selected" : ""}`} onClick={() => onFamily(option)}><i className="preference-radio"/><span><strong>{option === "ge-proton" ? "GE-Proton" : "Proton CachyOS"}</strong><small>{option === "ge-proton" ? "GloriousEggroll" : "CachyOS"}</small></span></button>)}</div>
    <div className="preference-action-right"><button type="button" className="preference-outline" disabled={disabled} onClick={onUpdate}><MenuIcon name="download"/>{text.runnerUpdate}</button></div>
    <section className="preference-section"><h3>{text.steam}</h3><label className="preference-switch"><input ref={focusRef} type="checkbox" checked={Boolean(config.scan_steam_protons)} disabled={disabled} onChange={event => save({ scan_steam_protons: event.target.checked })}/><span className="preference-switch-track"/><span>{text.steamScan}<small>{text.steamHint}</small></span></label></section>
  </>;
}
export function CoverPreferences({ config, disabled, save, focusRef, onError }: ConfigProps & { focusRef: RefObject<HTMLInputElement | null>; onError: (error: string) => void }) {
  const text = usePreferencesText(), { t } = useI18n();
  const [show, setShow] = useState(false), [draft, setDraft] = useState(config.steamgriddb_api_key ?? "");
  return <><h2>{text.covers}</h2><section className="preference-section first"><h3>{t("steamgrid", "title")}</h3><p>{t("steamgrid", "description")}</p><label htmlFor="steamgrid-api-key">{t("steamgrid", "keyLabel")}</label>
    <div className="steamgrid-key-controls"><input ref={focusRef} id="steamgrid-api-key" type={show ? "text" : "password"} autoComplete="off" spellCheck={false} maxLength={128} disabled={disabled} value={draft} placeholder={t("steamgrid", "placeholder")} onChange={event => { setDraft(event.target.value); save({ steamgriddb_api_key: event.target.value.trim() }); }}/><button type="button" aria-pressed={show} aria-label={show ? text.hideKey : text.showKey} onClick={() => setShow(!show)}>{t("steamgrid", show ? "hide" : "show")}</button><button type="button" disabled={disabled || !draft} onClick={() => { setDraft(""); save({ steamgriddb_api_key: "" }); }}>{t("steamgrid", "remove")}</button></div>
    <div className="steamgrid-settings-footer"><small>{t("steamgrid", "localOnly")}</small><button type="button" onClick={() => void openUrl("https://www.steamgriddb.com/profile/preferences/api").catch(error => onError(String(error)))}>{t("steamgrid", "getKey")} ↗</button></div><p>{t("steamgrid", "rescanHint")}</p>
  </section></>;
}
export function ToolsPreferences({ onError }: { onError: (error: string) => void }) {
  const text = usePreferencesText(), { t } = useI18n();
  const [folders, setFolders] = useState<string[]>(() => { try { const value = JSON.parse(localStorage.getItem("custom_folders") ?? "[]"); return Array.isArray(value) ? value.filter(item => typeof item === "string") : []; } catch { return []; } });
  const remove = (folder: string) => { const next = folders.filter(item => item !== folder); try { localStorage.setItem("custom_folders", JSON.stringify(next)); setFolders(next); window.dispatchEvent(new Event("refreshGames")); } catch (error) { onError(String(error)); } };
  return <><h2>{text.tools}</h2><section className="preference-section first"><h3>{text.openLogs}</h3><p>{text.logsHint}</p><button type="button" className="preference-outline" onClick={() => void invoke("collect_and_open_logs", { gameName: "", gameDir: "" }).catch(error => onError(String(error)))}><MenuIcon name="logs"/>{text.openLogs}</button></section>
    <section className="preference-section"><h3>{t("settings", "manualDirs")}</h3><p>{t("settings", "manualDirsDesc")}</p>{folders.length ? <div className="preference-folders">{folders.map(folder => <div key={folder}><span>{folder}</span><button type="button" aria-label={`${t("settings", "removeDir")}: ${folder}`} onClick={() => remove(folder)}><MenuIcon name="trash"/></button></div>)}</div> : <p>{text.noFolders}</p>}</section>
  </>;
}

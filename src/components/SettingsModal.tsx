import { useEffect, useRef, useState } from "react";
import type { AppConfig } from "./SetupWizard";
import { invoke } from "@tauri-apps/api/core";
import { LauncherLogsModal } from "./LauncherLogsModal";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { ConfirmModal } from "./ConfirmModal";
import { ResultModal } from "./ResultModal";
import { BackendUpdaterModal } from "./BackendUpdaterModal";
import { ThemePreferences, ModPreferences, ProtonPreferences, CoverPreferences, ToolsPreferences } from "./preferences/PreferencePages";
import { usePreferencesConfig } from "./preferences/usePreferencesConfig";
import { usePreferencesText } from "./preferences/labels";
import { ConnectionsPreferences } from "./preferences/ConnectionsPreferences";
import "./preferences/preferences.css";

interface Props {
  onClose: () => void;
  onBusyChange?: (busy: boolean) => void;
  initialSection?: "general" | "covers" | "protons" | "backendUpdate";
  initialBackendArch?: "rdna3" | "rdna4";
  onConfigUpdated: (config: AppConfig | null) => void;
  onOpenWizard: () => void;
}
type Page = "home" | "interface" | "mods" | "protons" | "covers" | "tools" | "connections" | "backendUpdate";
export function SettingsModal({ onClose, initialSection = "general", initialBackendArch, onBusyChange, onConfigUpdated, onOpenWizard }: Props) {
  const text = usePreferencesText(), { t } = useI18n();
  const preferences = usePreferencesConfig(onConfigUpdated);
  const [page, setPage] = useState<Page>(initialSection === "general" ? "home" : initialSection);
  const [wideLayout, setWideLayout] = useState(() => window.matchMedia("(min-width: 820px)").matches);
  useEffect(() => {
    const media = window.matchMedia("(min-width: 820px)");
    const update = () => setWideLayout(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  const displayedPage = page === "home" && wideLayout ? "interface" : page;
  const [backendArch, setBackendArch] = useState(initialBackendArch);
  const [downloadBusy, setDownloadBusy] = useState(false), [actionError, setActionError] = useState("");
  const [showLauncherLogs, setShowLauncherLogs] = useState(false);
  const [notice, setNotice] = useState("");
  const [search, setSearch] = useState("");
  const [searchTarget, setSearchTarget] = useState<string | null>(null);
  const pageRef = useRef<HTMLDivElement>(null);
  const [cacheDialog, setCacheDialog] = useState<"confirm" | "success" | "error" | null>(null);
  const [clearingCache, setClearingCache] = useState(false), [cacheError, setCacheError] = useState("");
  const steamRef = useRef<HTMLInputElement>(null), coverRef = useRef<HTMLInputElement>(null), wasSaving = useRef(false);
  const busy = preferences.saving || downloadBusy || clearingCache;
  useEffect(() => { setBackendArch(initialBackendArch); setPage(initialSection === "general" ? "home" : initialSection); }, [initialSection, initialBackendArch]);
  useEffect(() => { onBusyChange?.(busy); return () => onBusyChange?.(false); }, [busy, onBusyChange]);
  const close = () => { if (!busy && !preferences.isPending() && !cacheDialog && !showLauncherLogs) onClose(); };
  const parent: Page = page === "backendUpdate" ? "mods" : "home";
  const back = () => { if (!busy && !preferences.isPending()) { setPage(parent); setActionError(""); } };
  useEffect(() => {
    if (wasSaving.current && !preferences.saving && !preferences.error) setNotice(text.saved);
    wasSaving.current = preferences.saving;
  }, [preferences.saving, preferences.error, text.saved]);
  useEffect(() => { if (notice) { const timer = window.setTimeout(() => setNotice(""), 3000); return () => window.clearTimeout(timer); } }, [notice]);
  useEffect(() => {
    if (preferences.ready) {
      if (initialSection === "covers" && page === "covers") coverRef.current?.focus({ preventScroll: true });
      if (initialSection === "protons" && page === "protons") steamRef.current?.focus({ preventScroll: true });
    }
  }, [preferences.ready, initialSection, page]);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !cacheDialog && !showLauncherLogs) { event.stopImmediatePropagation(); if (search) { setSearch(""); return; } if (!busy && !preferences.isPending()) { if (page === "home" || (wideLayout && parent === "home")) onClose(); else setPage(parent); } }
    };
    window.addEventListener("keydown", escape, true); return () => window.removeEventListener("keydown", escape, true);
  }, [busy, cacheDialog, showLauncherLogs, page, parent, onClose, search, wideLayout]);
  const clearCache = async () => {
    if (clearingCache) return;
    setClearingCache(true);
    try { await invoke("clear_game_caches"); window.dispatchEvent(new Event("gameInfoCacheCleared")); setCacheDialog("success"); }
    catch (error) { setCacheError(String(error)); setCacheDialog("error"); }
    finally { setClearingCache(false); }
  };
  const categoryNames = { home: text.home, interface: text.interface, mods: text.mods, protons: text.protons, covers: text.covers, tools: text.tools, connections: text.connections, backendUpdate: t("app", "updateBackend") };
  const categories = [
    { page: "interface", icon: "platform", hint: text.interfaceHint },
    { page: "mods", icon: "puzzle", hint: text.modsHint },
    { page: "protons", icon: "cube", hint: text.protonsHint },
    { page: "covers", icon: "image", hint: text.coversHint },
    { page: "connections", icon: "chainLinked", hint: text.connectionsHint },
    { page: "tools", icon: "tools", hint: text.toolsHint },
  ] as const;
  const normalize = (value: string) => value.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const query = normalize(search.trim());
  // Index each real control separately; category names provide context.
  const options = [
    { id: "connection-nexus", page: "connections", title: "Nexus Mods", keywords: text.connectionsSearch },
    { id: "connection-discord", page: "connections", title: "Discord", keywords: `${text.connectionsSearch} YouTube` },
    { id: "connection-matrix", page: "connections", title: "Matrix", keywords: text.connectionsSearch },
    { id: "themes", page: "interface", title: text.themes, keywords: `${text.darkHint} ${text.lightHint}` },
    { id: "accent", page: "interface", title: text.accent, keywords: "accent color" },
    { id: "blur-windows", page: "interface", title: text.windowsEffect, keywords: text.blurSearch },
    { id: "blur-backdrop", page: "interface", title: text.backdropEffect, keywords: text.blurSearch },
    { id: "reset-glass", page: "interface", title: text.resetGlass, keywords: "" },
    { id: "launch-effect", page: "interface", title: text.launchEffect, keywords: `${text.freezeEffect} ${text.freezeHint} ${text.shatterEffect} ${text.smoothEffect} ${text.aquariumEffect} ${text.aquariumHint} ${text.particlesEffect} ${text.particlesHint} ${text.penguinEffect} ${text.penguinHint} ${text.shrinkEffect} ${text.shrinkHint} ${text.noLaunchEffect} ${text.launchEffectHint}` },
    { id: "backend-project", page: "mods", title: text.project, keywords: `${text.rdna4} ${text.rdna3} DLSSNR AMD OptiScaler` },
    { id: "backend-update", page: "mods", title: text.backendUpdate, keywords: text.backendHint },
    { id: "setup", page: "mods", title: t("settings", "reset"), keywords: text.modTools },
    { id: "clear-cache", page: "mods", title: t("gameCache", "clear"), keywords: "cache" },
    { id: "proton-family", page: "protons", title: text.distribution, keywords: `GE-Proton CachyOS GloriousEggroll ${text.runnerUpdate} ${t("runners", "download")} ${t("runners", "check")}` },
    { id: "steam-scan", page: "protons", title: text.steamScan, keywords: `${text.steam} ${text.steamHint}` },
    { id: "cover-api", page: "covers", title: t("steamgrid", "keyLabel"), keywords: `SteamGridDB API ${text.showKey} ${text.hideKey}` },
    { id: "launcher-logs", page: "tools", title: text.launcherLogs, keywords: `${text.openLogs} ${text.logsHint}` },
    { id: "backup-create", page: "tools", title: text.backupCreate, keywords: "Nexus ZIP" },
    { id: "backup-restore", page: "tools", title: text.backupRestore, keywords: text.backupSearch },
    { id: "backup-discover", page: "tools", title: text.backupDiscover, keywords: text.backupSearch },
    { id: "backup-per-game", page: "tools", title: text.backupPerGame, keywords: text.backupSearch },
    { id: "manual-folders", page: "tools", title: t("settings", "manualDirs"), keywords: text.directorySearch },
  ] as const;
  const searchResults = options.filter(option => query.split(/\s+/).every(word => normalize(`${categoryNames[option.page]} ${option.title} ${option.keywords}`).includes(word)));
  useEffect(() => {
    if (!searchTarget || query || !preferences.ready) return;
    const target = pageRef.current?.querySelector<HTMLElement>(`[data-preference-id="${searchTarget}"]`);
    if (!target) return;
    target.scrollIntoView({ block: "center", behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
    const control = target.matches("button,input") ? target : target.querySelector<HTMLElement>("input,button");
    control?.focus({ preventScroll: true });
    target.classList.add("preference-search-target");
    const timer = window.setTimeout(() => { target.classList.remove("preference-search-target"); setSearchTarget(null); }, 1800);
    return () => { window.clearTimeout(timer); target.classList.remove("preference-search-target"); };
  }, [searchTarget, query, displayedPage, preferences.ready]);
  const configProps = { config: preferences.config, disabled: !preferences.ready || busy, save: preferences.save };
  const footer = <footer className="preferences-footer"><span role="status"><MenuIcon name="info"/>{preferences.saving ? text.saving : notice || text.automatic}</span></footer>;
  return <div className="preferences-overlay">
    <div className="dialog-backdrop" onClick={close}/>
    {showLauncherLogs && <LauncherLogsModal onClose={() => setShowLauncherLogs(false)}/>}
    {cacheDialog === "confirm" && <ConfirmModal title={t("gameCache", "title")} message={t("gameCache", "confirm")} confirmText={t("gameCache", "clear")} cancelText={t("confirm", "cancel")} busy={clearingCache} onConfirm={() => void clearCache()} onCancel={() => setCacheDialog(null)}/>}
    {(cacheDialog === "success" || cacheDialog === "error") && <ResultModal title={t("gameCache", cacheDialog === "success" ? "successTitle" : "errorTitle")} message={cacheDialog === "success" ? t("gameCache", "success") : t("gameCache", "error") + cacheError} type={cacheDialog === "success" ? "success" : "error"} logs="" showLogButton={false} onClose={() => setCacheDialog(null)}/>}
    <ModalSurface className="modal-content dialog-glass preferences-modal" footer={footer} onDismiss={close} closeDisabled={busy} role="dialog" aria-modal="true" aria-label={t("settings", "title")} headerActions={<label className="advanced-search preferences-search"><MenuIcon name="search"/><input type="search" disabled={busy} aria-label={text.searchSettings} placeholder={text.searchPlaceholder} value={search} onChange={event => setSearch(event.target.value)}/></label>} header={<nav className="preferences-breadcrumb" aria-label={text.home}>
      {displayedPage !== "home" && (!wideLayout || parent !== "home") && <button type="button" onClick={back} disabled={busy} aria-label={text.back}><MenuIcon name="back"/></button>}
      <h2 className="preferences-home-title"><MenuIcon name="settings" />{text.home}</h2>
      {displayedPage !== "home" && <><span>/</span><strong>{categoryNames[displayedPage]}</strong></>}

    </nav>}>
      <div className="preferences-layout">
        {wideLayout && <nav className="preferences-sidebar preferences-categories" aria-label={text.home}>
          {categories.map(category => <button type="button" key={category.page} disabled={busy} aria-current={(displayedPage === category.page || (displayedPage === "backendUpdate" && category.page === "mods")) ? "page" : undefined} onClick={() => { if (!preferences.isPending()) { setPage(category.page); setSearch(""); setSearchTarget(null); setActionError(""); } }}><span className="preferences-category-icon"><MenuIcon name={category.icon}/></span><span><strong>{categoryNames[category.page]}</strong><small>{category.hint}</small></span></button>)}
        </nav>}
        <div ref={pageRef} className="preferences-page" key={displayedPage}>

        {query ? <><h2>{text.searchResults}</h2><div className="preferences-categories">
          {searchResults.map(option => <button type="button" key={option.id} disabled={busy} onClick={() => { if (!preferences.isPending()) { setPage(option.page); setSearch(""); setSearchTarget(option.id); setActionError(""); } }}><span className="preferences-category-icon"><MenuIcon name={categories.find(category => category.page === option.page)!.icon}/></span><span><strong>{option.title}</strong><small>{categoryNames[option.page]}</small></span><span className="preferences-chevron">›</span></button>)}
          {!searchResults.length && <p role="status">{text.searchEmpty}</p>}
        </div></> : <>
        {displayedPage === "home" && <div className="preferences-categories">{categories.map(category => <button type="button" key={category.page} onClick={() => { setPage(category.page); setActionError(""); }}><span className="preferences-category-icon"><MenuIcon name={category.icon}/></span><span><strong>{categoryNames[category.page]}</strong><small>{category.hint}</small></span><span className="preferences-chevron">›</span></button>)}</div>}
        {displayedPage === "interface" && <ThemePreferences onApplied={() => setNotice(text.applied)} onOpenConnections={() => { if (!busy && !preferences.isPending()) { setPage("connections"); setSearch(""); setSearchTarget(null); setActionError(""); } }}/>}
        {displayedPage === "connections" && <ConnectionsPreferences/>}
        {displayedPage === "mods" && <ModPreferences {...configProps} onUpdate={architecture => { setBackendArch(architecture); setPage("backendUpdate"); }} onWizard={() => { if (!preferences.isPending()) { onOpenWizard(); onClose(); } }} onClear={() => setCacheDialog("confirm")}/>}
        {displayedPage === "protons" && <ProtonPreferences {...configProps} onBusy={setDownloadBusy} focusRef={steamRef}/>}
        {displayedPage === "covers" && preferences.ready && <CoverPreferences {...configProps} disabled={!preferences.ready} focusRef={coverRef} onError={setActionError}/>}
        {displayedPage === "tools" && <ToolsPreferences onRestored={async () => onConfigUpdated(await invoke<AppConfig | null>("load_app_config"))} onBusy={setDownloadBusy} onError={setActionError} onLogs={() => setShowLauncherLogs(true)}/>}
        {displayedPage === "backendUpdate" && <BackendUpdaterModal key={backendArch ?? preferences.config.backend} gpuArch={backendArch ?? (preferences.config.backend === "AMDNR" ? "rdna4" : "rdna3")} isEmbedded onBusyChange={setDownloadBusy} onClose={() => setPage("mods")}/>}
        {(preferences.error || actionError) && <div className="preferences-error" role="alert"><p>{preferences.error || actionError}</p>{preferences.error && <button type="button" disabled={busy} onClick={preferences.retry}>{text.retry}</button>}</div>}
        </>}
      </div>
      </div>

    </ModalSurface>
  </div>;
}

import { MonitoringPreferences } from "./preferences/MonitoringPreferences";
import { useEffect, useRef, useState } from "react";
import type { AppConfig } from "./SetupWizard";
import { invoke } from "@tauri-apps/api/core";
import { LauncherLogsModal } from "./LauncherLogsModal";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { ConfirmModal } from "./ConfirmModal";
import { ResultModal } from "./ResultModal";
import { BackendUpdaterModal } from "./BackendUpdaterModal";
import { ThemePreferences, ModPreferences, ProtonPreferences, CoverPreferences, ToolsPreferences } from "./preferences/PreferencePages";
import { usePreferencesConfig } from "./preferences/usePreferencesConfig";
import { usePreferencesText } from "./preferences/labels";
import { ConnectionsPreferences } from "./preferences/ConnectionsPreferences";
import { ModBackupPreferences } from "./preferences/ModBackupPreferences";
import { DiskAutomountPreferences } from "./preferences/DiskAutomountPreferences";
import "./preferences/preferences.css";
import "./preferences/modernPreferences.css";

interface Props {
  onClose: () => void;
  onBusyChange?: (busy: boolean) => void;
  initialSection?: "general" | "covers" | "protons" | "backendUpdate" | "diskAutomount" | "monitoring" | "interface" | "connections" | "nexusAccount" | "appimageInstall";
  initialBackendArch?: "rdna3" | "rdna4";
  onConfigUpdated: (config: AppConfig | null) => void;
  onOpenWizard: () => void;
  onOpenWelcome: () => void;
  /** Settings are a page of the window; its side menu picks the page. */
  page?: SettingsPage;
  onPageChange?: (page: SettingsPage) => void;
}
export type SettingsPage = Page;
type Page = "home" | "interface" | "mods" | "protons" | "covers" | "tools" | "connections" | "backendUpdate" | "diskAutomount" | "monitoring";
export function SettingsModal({ onClose, initialSection = "general", initialBackendArch, onBusyChange, onConfigUpdated, onOpenWizard, onOpenWelcome, page: controlledPage, onPageChange }: Props) {
  const text = usePreferencesText(), { t } = useI18n();
  const preferences = usePreferencesConfig(onConfigUpdated);
  const initialPage: Page = initialSection === "general" ? "home" : initialSection === "appimageInstall" ? "tools" : initialSection === "nexusAccount" ? "connections" : initialSection;
  const [ownPage, setOwnPage] = useState<Page>(initialPage);
  const page = controlledPage ?? ownPage;
  const setPage = (next: Page) => { if (onPageChange) onPageChange(next); else setOwnPage(next); };
  const displayedPage = page === "home" ? "interface" : page;
  const [backendArch, setBackendArch] = useState(initialBackendArch);
  const [protonConfirmationOpen, setProtonConfirmationOpen] = useState(false);
  const [downloadBusy, setDownloadBusy] = useState(false), [actionError, setActionError] = useState("");
  const [storageDialogOpen, setStorageDialogOpen] = useState(false);
  const [showLauncherLogs, setShowLauncherLogs] = useState(false);
  const [notice, setNotice] = useState("");
  const [search, setSearch] = useState("");
  const [searchExpanded, setSearchExpanded] = useState(false);
  const searchInput = useRef<HTMLInputElement | null>(null), searchFocused = useRef(false);
  const [searchTarget, setSearchTarget] = useState<string | null>(initialSection === "appimageInstall" ? "appimage-install" : initialSection === "nexusAccount" ? "connection-nexus" : null);
  const pageRef = useRef<HTMLDivElement>(null);
  const [cacheDialog, setCacheDialog] = useState<"confirm" | "success" | "error" | null>(null);
  const [clearingCache, setClearingCache] = useState(false), [cacheError, setCacheError] = useState("");
  const steamRef = useRef<HTMLInputElement>(null), coverRef = useRef<HTMLInputElement>(null), wasSaving = useRef(false);
  const busy = preferences.saving || downloadBusy || clearingCache;
  useEffect(() => { setBackendArch(initialBackendArch); setPage(initialPage); setSearchTarget(initialSection === "appimageInstall" ? "appimage-install" : initialSection === "nexusAccount" ? "connection-nexus" : null); }, [initialPage, initialSection, initialBackendArch]);
  useEffect(() => { onBusyChange?.(busy); return () => onBusyChange?.(false); }, [busy, onBusyChange]);
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
      if (document.querySelector("[data-shortcut-recording]")) return;
      if (document.querySelector(".modern-drawer-open, .maintenance-popup")) return;
      // A portaled choice menu consumes Escape before navigating out of preferences.
      if (event.target instanceof Element && (event.target.closest('.custom-select-menu[role="listbox"]') || (event.target.getAttribute("aria-haspopup") === "listbox" && event.target.getAttribute("aria-expanded") === "true"))) return;
      if (event.key === "Escape" && !storageDialogOpen && !cacheDialog && !showLauncherLogs && !protonConfirmationOpen) { event.stopImmediatePropagation(); if (search) { setSearch(""); return; } if (!busy && !preferences.isPending()) { if (parent === "home") onClose(); else setPage(parent); } }
    };
    window.addEventListener("keydown", escape, true); return () => window.removeEventListener("keydown", escape, true);
  }, [busy, storageDialogOpen, cacheDialog, showLauncherLogs, protonConfirmationOpen, page, parent, onClose, search]);
  const clearCache = async () => {
    if (clearingCache) return;
    setClearingCache(true);
    try { await invoke("clear_game_caches"); window.dispatchEvent(new Event("gameInfoCacheCleared")); setCacheDialog("success"); }
    catch (error) { setCacheError(String(error)); setCacheDialog("error"); }
    finally { setClearingCache(false); }
  };
  const categoryNames = { home: text.home, interface: text.interface, mods: text.mods, protons: text.protons, covers: text.covers, tools: text.tools, diskAutomount: text.diskAutomount, monitoring: text.monitoring, connections: text.connections, backendUpdate: t("app", "updateBackend") };
  // Search also reaches nested pages that are absent from the main categories.
  // Requiring every Page here makes new destinations a type-checked addition.
  const pageIcons = {
    home: "home", interface: "platform", mods: "puzzle", protons: "cube",
    covers: "image", connections: "chainLinked", tools: "tools",
    diskAutomount: "folder", monitoring: "graphics", backendUpdate: "download",
  } as const satisfies Record<Page, React.ComponentProps<typeof MenuIcon>["name"]>;
  const normalize = (value: string) => value.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const query = normalize(search.trim());
  // Index each real control separately; category names provide context.
  const options = [
    { id: "interface-theme", page: "interface", title: "Hackerman", keywords: "temas theme Matrix Fallout retro hacker terminal verde green pixel" },
    { id: "connection-nexus", page: "connections", title: "Nexus Mods", keywords: text.connectionsSearch },
    { id: "connection-discord", page: "connections", title: "Discord", keywords: `${text.connectionsSearch} YouTube` },
    { id: "connection-matrix", page: "connections", title: "Matrix", keywords: text.connectionsSearch },
    { id: "themes", page: "interface", title: text.themes, keywords: `${text.darkHint} ${text.lightHint}` },
    { id: "accent", page: "interface", title: text.accentAndBlur, keywords: "accent color" },
    { id: "blur-windows", page: "interface", title: text.windowsEffect, keywords: text.blurSearch },
    { id: "launch-effect", page: "interface", title: text.launchEffect, keywords: `${text.freezeEffect} ${text.freezeHint} ${text.shatterEffect} ${text.smoothEffect} ${text.aquariumEffect} ${text.aquariumHint} ${text.particlesEffect} ${text.particlesHint} ${text.penguinEffect} ${text.penguinHint} ${text.shrinkEffect} ${text.shrinkHint} ${text.noLaunchEffect} ${text.launchEffectHint}` },
    { id: "backend-project", page: "mods", title: text.project, keywords: `${text.rdna4} ${text.rdna3} DLSSNR AMD OptiScaler` },
    { id: "backend-update", page: "mods", title: text.backendUpdate, keywords: text.backendHint },
    { id: "setup", page: "mods", title: t("settings", "reset"), keywords: text.modTools },
    { id: "clear-cache", page: "mods", title: t("gameCache", "clear"), keywords: "cache" },
    { id: "proton-family", page: "protons", title: text.distribution, keywords: `GE-Proton CachyOS GloriousEggroll ${text.runnerUpdate} ${t("runners", "download")} ${t("runners", "check")}` },
    { id: "steam-scan", page: "protons", title: text.steamScan, keywords: `${text.steam} ${text.steamHint}` },
    { id: "cover-api", page: "covers", title: t("steamgrid", "keyLabel"), keywords: `SteamGridDB API ${text.showKey} ${text.hideKey}` },
    { id: "disk-automount", page: "diskAutomount", title: text.diskAutomount, keywords: `${text.diskAutomountHint} HD HDD SSD storage GNOME Disks` },
    { id: "welcome-tour", page: "tools", title: t("welcome", "title"), keywords: "tour wizard boas-vindas apresentação primeiros passos recursos atalhos welcome getting started features shortcuts" },
    { id: "experimental-updates", page: "tools", title: text.experimentalUpdates, keywords: `${text.launcherUpdates} ${text.experimentalUpdatesHint} beta update atualização` },
    { id: "monitoring-panel", page: "monitoring", title: text.monitoring, keywords: `${text.monitoringHint} FPS GPU CPU Goverlay` },
    { id: "launcher-logs", page: "tools", title: text.launcherLogs, keywords: `${text.openLogs} ${text.logsHint}` },
    { id: "backup-create", page: "mods", title: text.backupCreate, keywords: "Nexus ZIP" },
    { id: "backup-restore", page: "mods", title: text.backupRestore, keywords: text.backupSearch },
    { id: "backup-discover", page: "mods", title: text.backupDiscover, keywords: text.backupSearch },
    { id: "backup-per-game", page: "mods", title: text.backupPerGame, keywords: text.backupSearch },
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
  const searchControl = <div className="settings-page-search" data-expanded={searchExpanded || undefined}
    onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget)) { searchFocused.current = false; setSearchExpanded(false); } }}>
    <button type="button" disabled={busy} aria-label={text.searchSettings} aria-expanded={searchExpanded} onClick={() => { setSearchExpanded(true); requestAnimationFrame(() => searchInput.current?.focus()); }}><MenuIcon name="search"/></button>
    <input ref={node => { searchInput.current = node; if (node && searchFocused.current) node.focus({ preventScroll: true }); }} type="search" disabled={busy} aria-label={text.searchSettings} placeholder={text.searchPlaceholder} value={search}
      onFocus={() => { searchFocused.current = true; }} onChange={event => setSearch(event.target.value)} onKeyDown={event => { if (event.key === "Escape") { setSearch(""); setSearchExpanded(false); searchFocused.current = false; event.currentTarget.blur(); } }}/>
  </div>;
  const pageBody = <>
        {query ? <><h2>{text.searchResults}</h2><div className="preferences-categories">
          {searchResults.map(option => <button type="button" key={option.id} data-page={option.page} disabled={busy} onClick={() => { if (!preferences.isPending()) { setPage(option.page); setSearch(""); setSearchTarget(option.id); setActionError(""); } }}><span className="preferences-category-icon"><MenuIcon name={pageIcons[option.page]}/></span><span><strong>{option.title}</strong><small>{categoryNames[option.page]}</small></span><span className="preferences-chevron">›</span></button>)}
          {!searchResults.length && <p role="status">{text.searchEmpty}</p>}
        </div></> : <>
        {displayedPage === "interface" && <ThemePreferences onApplied={() => setNotice(text.applied)} onOpenConnections={() => { if (!busy && !preferences.isPending()) { setPage("connections"); setSearch(""); setSearchTarget(null); setActionError(""); } }}/>}
        {displayedPage === "connections" && <ConnectionsPreferences/>}
        {displayedPage === "mods" && <><ModPreferences {...configProps} onUpdate={architecture => { setBackendArch(architecture); setPage("backendUpdate"); }} onWizard={() => { if (!preferences.isPending()) { onOpenWizard(); onClose(); } }} onClear={() => setCacheDialog("confirm")}/><ModBackupPreferences onError={setActionError} onBusy={setDownloadBusy} onRestored={async () => onConfigUpdated(await invoke<AppConfig | null>("load_app_config"))}/></>}
        {displayedPage === "protons" && <ProtonPreferences {...configProps} onBusy={setDownloadBusy} focusRef={steamRef} onConfirmationChange={setProtonConfirmationOpen}/>}
        {displayedPage === "covers" && preferences.ready && <CoverPreferences {...configProps} disabled={!preferences.ready} focusRef={coverRef} onError={setActionError}/>}
        {displayedPage === "tools" && <ToolsPreferences onWelcome={() => { if (!busy && !preferences.isPending()) { onClose(); onOpenWelcome(); } }} onError={setActionError} onLogs={() => setShowLauncherLogs(true)}/>}
        {displayedPage === "diskAutomount" && <DiskAutomountPreferences onBusy={setDownloadBusy} onDialogChange={setStorageDialogOpen}/>}
        {displayedPage === "backendUpdate" && <section className="preference-panel"><BackendUpdaterModal key={backendArch ?? preferences.config.backend} gpuArch={backendArch ?? (preferences.config.backend === "AMDNR" ? "rdna4" : "rdna3")} isEmbedded onBusyChange={setDownloadBusy} onClose={() => setPage("mods")}/></section>}
        {(preferences.error || actionError) && <div className="preferences-error" role="alert"><p>{preferences.error || actionError}</p>{preferences.error && <button type="button" disabled={busy} onClick={preferences.retry}>{text.retry}</button>}</div>}
        </>}
  </>;
  const dialogs = <>
    {showLauncherLogs && <LauncherLogsModal onClose={() => setShowLauncherLogs(false)}/>}
    {cacheDialog === "confirm" && <ConfirmModal title={t("gameCache", "title")} message={t("gameCache", "confirm")} confirmText={t("gameCache", "clear")} cancelText={t("confirm", "cancel")} busy={clearingCache} onConfirm={() => void clearCache()} onCancel={() => setCacheDialog(null)}/>}
    {(cacheDialog === "success" || cacheDialog === "error") && <ResultModal title={t("gameCache", cacheDialog === "success" ? "successTitle" : "errorTitle")} message={cacheDialog === "success" ? t("gameCache", "success") : t("gameCache", "error") + cacheError} type={cacheDialog === "success" ? "success" : "error"} logs="" showLogButton={false} onClose={() => setCacheDialog(null)}/>}
  </>;
  const hints: Partial<Record<Page, string>> = { interface: text.interfaceHint, mods: text.modsHint, protons: text.protonsHint, covers: text.coversHint, connections: text.connectionsHint, tools: text.toolsHint, diskAutomount: text.diskAutomountHint };
  return <div className={`settings-page${query ? " settings-searching" : ""}`} aria-label={t("settings", "title")}>
    {dialogs}
    {displayedPage !== "monitoring" && <header className="settings-page-head">
      {parent !== "home" && <button type="button" className="settings-page-back" onClick={back} disabled={busy} aria-label={text.back}><MenuIcon name="back"/></button>}
      <div><h1>{query ? text.searchResults : categoryNames[displayedPage]}</h1>{!query && hints[displayedPage] && <p>{hints[displayedPage]}</p>}</div>
      <span className="settings-page-spacer" />
      {searchControl}
    </header>}
    {displayedPage === "monitoring" && <MonitoringPreferences searchControl={searchControl} onDirtyChange={setDownloadBusy}/>}
    <div ref={pageRef} className="preferences-page settings-page-body" key={displayedPage}>{pageBody}</div>
    <footer className="settings-page-foot" role="status"><MenuIcon name="info"/>{preferences.saving ? text.saving : notice || (displayedPage === "monitoring" ? "As alterações do MangoHUD são salvas automaticamente." : text.automatic)}</footer>
  </div>;
}

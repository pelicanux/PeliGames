import { useEffect, useRef, useState } from "react";
import type { AppConfig } from "./SetupWizard";
import type { ProtonFamily } from "../services/protonService";
import { invoke } from "@tauri-apps/api/core";
import { LauncherLogsModal } from "./LauncherLogsModal";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { ConfirmModal } from "./ConfirmModal";
import { ResultModal } from "./ResultModal";
import { BackendUpdaterModal } from "./BackendUpdaterModal";
import { ProtonManagerModal } from "./ProtonManagerModal";
import { ThemePreferences, ModPreferences, ProtonPreferences, CoverPreferences, ToolsPreferences } from "./preferences/PreferencePages";
import { usePreferencesConfig } from "./preferences/usePreferencesConfig";
import { usePreferencesText } from "./preferences/labels";
import "./preferences/preferences.css";

interface Props {
  onClose: () => void;
  initialSection?: "general" | "covers" | "protons";
  onConfigUpdated: (config: AppConfig | null) => void;
  onOpenWizard: () => void;
}
type Page = "home" | "interface" | "mods" | "protons" | "covers" | "tools" | "backendUpdate" | "protonUpdate";
export function SettingsModal({ onClose, initialSection = "general", onConfigUpdated, onOpenWizard }: Props) {
  const text = usePreferencesText(), { t } = useI18n();
  const preferences = usePreferencesConfig(onConfigUpdated);
  const [page, setPage] = useState<Page>(initialSection === "general" ? "home" : initialSection);
  const [downloadBusy, setDownloadBusy] = useState(false), [actionError, setActionError] = useState("");
  const [showLauncherLogs, setShowLauncherLogs] = useState(false);
  const [notice, setNotice] = useState("");
  const [cacheDialog, setCacheDialog] = useState<"confirm" | "success" | "error" | null>(null);
  const [clearingCache, setClearingCache] = useState(false), [cacheError, setCacheError] = useState("");
  const steamRef = useRef<HTMLInputElement>(null), coverRef = useRef<HTMLInputElement>(null), wasSaving = useRef(false);
  const busy = preferences.saving || downloadBusy || clearingCache;
  const family: ProtonFamily = preferences.config.preferred_proton_family === "cachyos-proton" ? "cachyos-proton" : "ge-proton";
  const close = () => { if (!busy && !preferences.isPending() && !cacheDialog && !showLauncherLogs) onClose(); };
  const parent: Page = page === "backendUpdate" ? "mods" : page === "protonUpdate" ? "protons" : "home";
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
      if (event.key === "Escape" && !cacheDialog && !showLauncherLogs) { event.stopImmediatePropagation(); if (!busy && !preferences.isPending()) { if (page === "home") onClose(); else setPage(parent); } }
    };
    window.addEventListener("keydown", escape, true); return () => window.removeEventListener("keydown", escape, true);
  }, [busy, cacheDialog, showLauncherLogs, page, parent, onClose]);
  const clearCache = async () => {
    if (clearingCache) return;
    setClearingCache(true);
    try { await invoke("clear_game_caches"); window.dispatchEvent(new Event("gameInfoCacheCleared")); setCacheDialog("success"); }
    catch (error) { setCacheError(String(error)); setCacheDialog("error"); }
    finally { setClearingCache(false); }
  };
  const categoryNames = { home: text.home, interface: text.interface, mods: text.mods, protons: text.protons, covers: text.covers, tools: text.tools, backendUpdate: t("app", "updateBackend"), protonUpdate: text.runnerUpdate };
  const categories = [
    { page: "interface", icon: "platform", hint: text.interfaceHint },
    { page: "mods", icon: "puzzle", hint: text.modsHint },
    { page: "protons", icon: "cube", hint: text.protonsHint },
    { page: "covers", icon: "image", hint: text.coversHint },
    { page: "tools", icon: "tools", hint: text.toolsHint },
  ] as const;
  const configProps = { config: preferences.config, disabled: !preferences.ready || busy, save: preferences.save };
  const footer = <footer className="preferences-footer"><span role="status"><MenuIcon name="info"/>{preferences.saving ? text.saving : notice || (page === "mods" ? `${text.selected}: ${preferences.config.backend === "AMDNR" ? "DLSSNR-AMD" : "DLSSNR-RDNA3"}` : text.automatic)}</span></footer>;
  return <div className="preferences-overlay">
    <div className="dialog-backdrop" onClick={close}/>
    {showLauncherLogs && <LauncherLogsModal onClose={() => setShowLauncherLogs(false)}/>}
    {cacheDialog === "confirm" && <ConfirmModal title={t("gameCache", "title")} message={t("gameCache", "confirm")} confirmText={t("gameCache", "clear")} cancelText={t("confirm", "cancel")} busy={clearingCache} onConfirm={() => void clearCache()} onCancel={() => setCacheDialog(null)}/>}
    {(cacheDialog === "success" || cacheDialog === "error") && <ResultModal title={t("gameCache", cacheDialog === "success" ? "successTitle" : "errorTitle")} message={cacheDialog === "success" ? t("gameCache", "success") : t("gameCache", "error") + cacheError} type={cacheDialog === "success" ? "success" : "error"} logs="" showLogButton={false} onClose={() => setCacheDialog(null)}/>}
    <ModalSurface className="modal-content dialog-glass preferences-modal" footer={footer} onDismiss={close} closeDisabled={busy} role="dialog" aria-modal="true" aria-label={t("settings", "title")} header={<nav className="preferences-breadcrumb" aria-label={text.home}>
      {page !== "home" && <button type="button" onClick={back} disabled={busy} aria-label={text.back}><MenuIcon name="back"/></button>}
      {page === "home" ? <h2 className="preferences-home-title"><MenuIcon name="settings" />{text.home}</h2> : <span>{text.home}</span>}{page !== "home" && <><span>/</span>{(page === "backendUpdate" || page === "protonUpdate") && <><span>{categoryNames[parent]}</span><span>/</span></>}<strong>{categoryNames[page]}</strong>{page === "interface" && <><span>/</span><strong>{text.themes}</strong></>}</>}
    </nav>}>
      <div className="preferences-page" key={page}>
        {page === "home" && <div className="preferences-categories">{categories.map(category => <button type="button" key={category.page} onClick={() => { setPage(category.page); setActionError(""); }}><span className="preferences-category-icon"><MenuIcon name={category.icon}/></span><span><strong>{categoryNames[category.page]}</strong><small>{category.hint}</small></span><span className="preferences-chevron">›</span></button>)}</div>}
        {page === "interface" && <ThemePreferences onApplied={() => setNotice(text.applied)}/>}
        {page === "mods" && <ModPreferences {...configProps} onUpdate={() => setPage("backendUpdate")} onWizard={() => { if (!preferences.isPending()) { onOpenWizard(); onClose(); } }} onClear={() => setCacheDialog("confirm")}/>}
        {page === "protons" && <ProtonPreferences {...configProps} family={family} onFamily={preferred_proton_family => preferences.save({ preferred_proton_family })} onUpdate={() => setPage("protonUpdate")} focusRef={steamRef}/>}
        {page === "covers" && preferences.ready && <CoverPreferences {...configProps} disabled={!preferences.ready} focusRef={coverRef} onError={setActionError}/>}
        {page === "tools" && <ToolsPreferences onError={setActionError} onLogs={() => setShowLauncherLogs(true)}/>}
        {page === "backendUpdate" && <BackendUpdaterModal gpuArch={preferences.config.backend === "AMDNR" ? "rdna4" : "rdna3"} isEmbedded onBusyChange={setDownloadBusy} onClose={() => setPage("mods")}/>}
        {page === "protonUpdate" && <ProtonManagerModal initialFamily={family} isEmbedded hideNavigation onBusyChange={setDownloadBusy} onClose={() => setPage("protons")} onInstalled={() => window.dispatchEvent(new Event("protonRunnersChanged"))}/>}
        {(preferences.error || actionError) && <div className="preferences-error" role="alert"><p>{preferences.error || actionError}</p>{preferences.error && <button type="button" disabled={busy} onClick={preferences.retry}>{text.retry}</button>}</div>}
      </div>

    </ModalSurface>
  </div>;
}

import "./SetupWizard.css";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import React, { useState, useEffect } from 'react';
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { openDirectoryPicker } from "../services/tauriService";
import { ModelSourceSelector } from "./ModelSourceSelector";
import { ModelSource } from "../types/installer";

import { useI18n } from "../i18n/I18nContext";
import { hasDlssnrConfiguration } from "../services/modConfiguration";

export interface AppConfig {
  steamgriddb_api_key?: string;
  scan_steam_protons?: boolean;
  backend: string;
  dll_version: string;
  shortcut_key: string;
  game_shortcut_keys?: Record<string, string>;
  custom_game_paths?: Record<string, string>;
  preferred_proton_family?: "ge-proton" | "cachyos-proton";
}

interface DetectedGpu { model: string; pciAddress: string; backend: "rdna3" | "rdna4" | null; primary: boolean; }

interface Props {
  onComplete: (config: AppConfig) => void;
  allowCancel?: boolean;
  onCancel?: () => void;
  onBack?: () => void;
}

interface ProgressPayload {
  downloaded: number;
  total: number;
  speed_bytes_per_sec: number;
}

interface BackendVersionStatus {
  latest: string;
  current: string | null;
  needs_update: boolean;
}

export const SetupWizard: React.FC<Props> = ({ onComplete, allowCancel, onCancel, onBack }) => {
  const { t, language } = useI18n();
  const [gpuArch, setGpuArch] = useState<"rdna4" | "rdna3">("rdna4");
  const [gpus, setGpus] = useState<DetectedGpu[]>([]);
  const [detectingGpu, setDetectingGpu] = useState(true);
  const [gpuRevision, setGpuRevision] = useState(0);
  const recommendedGpu = gpus.find(gpu => gpu.backend);
  const gpuCompatible = !detectingGpu && !!recommendedGpu;

  useEffect(() => {
    let active = true;
    setDetectingGpu(true);
    invoke<DetectedGpu[]>("detect_linux_gpus").then(result => {
      if (!active) return;
      setGpus(result);
      const recommended = result.find(gpu => gpu.backend)?.backend;
      if (recommended) setGpuArch(recommended);
    }).catch(() => { if (active) setGpus([]); })
      .finally(() => { if (active) setDetectingGpu(false); });
    return () => { active = false; };
  }, [gpuRevision]);

  const [modelSource, setModelSource] = useState<ModelSource>("bin");
  const [dllPath, setDllPath] = useState("");
  const [binPath, setBinPath] = useState("");
  const [hasCachedBin, setHasCachedBin] = useState(false);
  const [cachedPaths, setCachedPaths] = useState<string[]>([]);
  const [backendPath, setBackendPath] = useState("");
  const [loading, setLoading] = useState(false);

  // Download state
  const [isDownloading, setIsDownloading] = useState(false);
  const [progressPct, setProgressPct] = useState(0);
  const [speedStr, setSpeedStr] = useState("");
  const [downloadComplete, setDownloadComplete] = useState(false);
  const [checkingVersion, setCheckingVersion] = useState(true);
  const [versionStatus, setVersionStatus] = useState<BackendVersionStatus | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [extractingDll, setExtractingDll] = useState(false);

  const readBackend = async () => {
    const state = await invoke<{path:string;ready:boolean;cached:string[]}>("get_backend_setup_status", {gpuArch});
    setBackendPath(state.ready ? state.path : "");
    setDownloadComplete(state.ready);
    setCachedPaths(state.cached); setHasCachedBin(state.cached.length > 0);
    setBinPath(state.cached[0] || "");
    return state;
  };
  useEffect(() => {
    let active = true;
    setVersionStatus(null); setDownloadComplete(false); setBackendPath("");
    setHasCachedBin(false); setCachedPaths([]); setBinPath(""); setDllPath("");
    if (!gpuCompatible) return;
    setCheckingVersion(true);
    const check = async () => {
      try {
        const state = await invoke<{path:string;ready:boolean;cached:string[]}>("get_backend_setup_status", {gpuArch});
        if (!active) return;
        setBackendPath(state.ready ? state.path : ""); setDownloadComplete(state.ready);
        setCachedPaths(state.cached); setHasCachedBin(state.cached.length > 0); setBinPath(state.cached[0] || "");
        const status = await invoke<BackendVersionStatus>("check_backend_version", {gpuArch}).catch(() => null);
        if (active) setVersionStatus(status);
      } catch (error) { if (active) setErrorMsg(String(error)); }
      finally { if (active) setCheckingVersion(false); }
    };
    void check();
    const unlisten = listen<ProgressPayload>("download-progress", ({payload}) => {
      if (!active) return;
      if (payload.total > 0) setProgressPct(Math.min(100, payload.downloaded / payload.total * 100));
      if (payload.speed_bytes_per_sec > 0) setSpeedStr(`${(payload.speed_bytes_per_sec / (1024 * 1024)).toFixed(1)} MB/s`);
    });
    return () => { active = false; unlisten.then(f => f()).catch(() => {}); };
  }, [gpuArch, gpuCompatible]);

  const handleDownload = async () => {
    if (!gpuCompatible || isDownloading) return;
    
    setIsDownloading(true);
    setProgressPct(0);
    setSpeedStr("");
    setDownloadComplete(false);
    setErrorMsg(null);
    
    try {
      await invoke<string>("update_backend", { gpuArch });
      const installed = await readBackend();
      if (!installed.ready) throw new Error(language === "pt" ? "Backend incompleto. Confira os arquivos baixados." : "Backend incomplete. Check the downloaded files.");
      const vStatus = await invoke<BackendVersionStatus>("check_backend_version", { gpuArch }).catch(() => null);
      if (vStatus) setVersionStatus(vStatus);
    } catch (err: any) {
      if (err === "Download cancelado pelo usuário.") {
        await readBackend().catch(() => {});
      } else {
        console.error(err);
        setErrorMsg(t("setupWizard", "downloadError") + err);
      }
    } finally {
      setIsDownloading(false);
    }
  };

  const handleCancelDownload = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await invoke("cancel_update");
  };

  const handleLocalFolder = async () => {
    if (!gpuCompatible) return;
    const path = await openDirectoryPicker(t("setupWizard", "selectLocalFolderPrompt"));
    if (!path) return;

    setLoading(true);
    setErrorMsg(null);
    try {
      await invoke<string>("copy_local_backend", { sourcePath: path, gpuArch });
      const installed = await readBackend();
      if (!installed.ready) throw new Error(language === "pt" ? "Backend incompleto. Confira os arquivos baixados." : "Backend incomplete. Check the downloaded files.");
      const vStatus = await invoke<BackendVersionStatus>("check_backend_version", { gpuArch }).catch(() => null);
      if (vStatus) setVersionStatus(vStatus);
    } catch (err) {
      console.error(err);
      setErrorMsg(`${t("setupWizard", "copyError")} ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!gpuCompatible) return;
    setErrorMsg(null);
    if (!downloadComplete) {
      setErrorMsg(t("setupWizard", "pleaseDownload"));
      return;
    }

    setLoading(true);
    // Give browser time to paint the loading state
    await new Promise(resolve => setTimeout(resolve, 50));

    try {
      let selectedPath = modelSource === "dll" ? dllPath : binPath;
      if (!selectedPath) {
        setErrorMsg(t("setupWizard", "selectFile"));
        setLoading(false);
        return;
      }
      
      if (modelSource === "dll") {
        setExtractingDll(true);
        // Give browser time to paint the extraction loading animation (two frames to be safe)
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(resolve, 150))));
        try {
          selectedPath = await invoke<string>("extract_dll_wizard", { dllPath: selectedPath, gpuArch });
        } catch (err: any) {
          setErrorMsg(t("setupWizard", "extractError") + err);
          setExtractingDll(false);
          setLoading(false);
          return;
        }
        setExtractingDll(false);
      }
      
      if (modelSource === "bin") selectedPath = await invoke<string>("import_model_bins", {gpuArch, binPath:selectedPath});
      await readBackend();
      const config: AppConfig = {
        backend: gpuArch === "rdna4" ? "AMDNR" : "OptiScaler",
        dll_version: selectedPath,
        shortcut_key: "Insert"
      };
      
      await invoke("save_app_config", { config });
      const saved = await invoke<AppConfig | null>("load_app_config");
      if (!hasDlssnrConfiguration(saved)) throw new Error("A configuração do mod não foi persistida.");
      onComplete(saved!);
    } catch (err) {
      console.error("Failed to save config:", err);
      setErrorMsg(`${t("setupWizard", "saveError")} ${String(err)}`)
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="modal-overlay setup-wizard-overlay">
      <ModalSurface className="modal-content dialog-glass setup-wizard setup-wizard-modern" role="dialog" aria-modal="true" aria-labelledby="setup-wizard-title"
        hideClose={!allowCancel || !onCancel} onDismiss={onCancel} closeDisabled={isDownloading || loading || extractingDll}
        header={<><h2 id="setup-wizard-title">{onBack ? <button type="button" className="wizard-titlebar-back" aria-label={t("welcome", "back")} disabled={isDownloading || loading || extractingDll} onClick={onBack}><MenuIcon name="back" /></button> : <MenuIcon name="sliders" />}{t("setupWizard", "title")}</h2></>}
        footer={        <footer className="setup-wizard-footer">
          <button
            onClick={handleSave}
            disabled={!gpuCompatible || loading || extractingDll || isDownloading || checkingVersion || !downloadComplete}
            className={`btn wizard-continue${loading || extractingDll ? " wizard-continue--busy" : ""}`}
            aria-busy={loading || extractingDll}
            style={{
              width: "100%", padding: "1rem", borderRadius: "8px", fontSize: "1.1rem",
              fontFamily: "'Rajdhani', sans-serif",
              background: "linear-gradient(to right, var(--accent-button), var(--accent-deep))", color: "var(--accent-foreground)", fontWeight: "bold",
              border: "none", cursor: (loading || extractingDll || !downloadComplete) ? "not-allowed" : "pointer",
              boxShadow: "0 4px 15px rgba(var(--accent-rgb), 0.3)",
              opacity: downloadComplete ? 1 : 0.5,
              display: "flex", justifyContent: "center", alignItems: "center", gap: "0.5rem",
              position: "relative",
              overflow: "hidden"
            }}
          >
            {(loading || extractingDll) && (
              <div style={{
                position: "absolute", left: 0, top: 0, bottom: 0,
                background: "rgba(var(--surface-255-255-255, 255, 255, 255), 0.2)",
                zIndex: 0,
                animation: "wizardModelProgress 60s linear forwards"
              }} />
            )}
            <div style={{ zIndex: 1, display: "flex", alignItems: "center", gap: "0.5rem", pointerEvents: "none" }}>
              {(loading || extractingDll) && (
                <svg className="spin" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="12" y1="2" x2="12" y2="6"></line>
                  <line x1="12" y1="18" x2="12" y2="22"></line>
                  <line x1="4.93" y1="4.93" x2="7.76" y2="7.76"></line>
                  <line x1="16.24" y1="16.24" x2="19.07" y2="19.07"></line>
                  <line x1="2" y1="12" x2="6" y2="12"></line>
                  <line x1="18" y1="12" x2="22" y2="12"></line>
                  <line x1="4.93" y1="19.07" x2="7.76" y2="16.24"></line>
                  <line x1="16.24" y1="7.76" x2="19.07" y2="4.93"></line>
                </svg>
              )}
              {extractingDll ? t("setupWizard", "extracting") : loading ? t("setupWizard", "saving") : t("setupWizard", "continue")}
            </div>
          </button>
        </footer>}>
        <p className="setup-wizard-subtitle">
          {t("setupWizard", "subtitle")}
        </p>
        
        <div className={`wizard-gpu-status${gpuCompatible ? " compatible" : ""}`} role="status" aria-live="polite">
          <MenuIcon name="chip" />
          <div>{detectingGpu ? t("setupWizard", "gpuChecking") : <>
            {gpus.length > 0 && <span><strong>{t("setupWizard", "gpuDetected")}:</strong> {gpus.map(gpu => gpu.model).join(" · ")}</span>}
            {!gpuCompatible && <span>{t("setupWizard", gpus.length ? "gpuIncompatible" : "gpuUnknown")}</span>}
          </>}</div>
          {!detectingGpu && !gpuCompatible && <button className="btn btn-secondary" onClick={() => setGpuRevision(value => value + 1)}>{t("setupWizard", "gpuRetry")}</button>}
        </div>
        <fieldset disabled={!gpuCompatible} style={{ border: 0, margin: 0, padding: 0, minWidth: 0, opacity: gpuCompatible ? 1 : 0.45 }}>
        <div className="backend-options wizard-backend-options">
          {([
            {arch:"rdna4", name:"DLSSNR-AMD", hardware:"RDNA 4 / RX 9000", author:"mochizuki0323", repo:"DLSSNR-AMD", description:"rdna4Desc"},
            {arch:"rdna3", name:"DLSSNR-RDNA3", hardware:"RDNA 3 / RX 7000", author:"mauri870", repo:"DLSSNR-RDNA3", description:"rdna3Desc"},
          ] as const).map(backend => <div key={backend.arch} className={`backend-option wizard-backend-card${gpuArch === backend.arch ? " active" : ""}`}>
            <button type="button" className="wizard-backend-select" aria-pressed={gpuArch === backend.arch}
              disabled={!gpuCompatible || isDownloading || loading} onClick={() => setGpuArch(backend.arch)}>
              <div className="wizard-backend-heading"><h3>{backend.name}</h3>
                <div className="wizard-backend-hardware"><span>{backend.hardware}</span>
                  {recommendedGpu?.backend === backend.arch && <small>✓ {t("setupWizard", "recommended")}</small>}
                </div>
              </div>
              <p>{t("setupWizard", backend.description)}</p>
            </button>
            <button type="button" className="wizard-maintainer-link" title={t("setupWizard", "viewOnGithub")}
              onClick={() => { void openUrl(`https://github.com/${backend.author}/${backend.repo}`); }}>
              {backend.author}<MenuIcon name="externalLink" />
            </button>
          </div>)}
        </div>
        <p className="wizard-backend-explanation">{language === "pt"
          ? "Este download inclui o backend DLSS NR e as ferramentas de extração e instalação. Não inclui a DLL da NVIDIA nem os modelos de IA: obtenha a nvngx_dlssnr.dll separadamente e extraia os modelos na etapa abaixo."
          : "This download includes the DLSS NR backend and extraction and installation tools. It does not include NVIDIA’s DLL or AI models: obtain nvngx_dlssnr.dll separately and extract the models below."}</p>

        {errorMsg && (
          <div style={{ fontSize: "0.85rem", color: "var(--tone-f87171, #f87171)", background: "rgba(248, 113, 113, 0.1)", padding: "0.8rem", borderRadius: "8px", border: "1px solid rgba(248, 113, 113, 0.2)", display: "flex", alignItems: "center", gap: "0.5rem", marginBottom: "1rem" }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><line x1="12" x2="12" y1="9" y2="13"/><line x1="12" x2="12.01" y1="17" y2="17"/></svg>
            <span style={{ whiteSpace: "pre-wrap" }}>{errorMsg}</span>
          </div>
        )}

        <div className="wizard-backend-download-actions">
          <button 
            className="btn wizard-download"
            style={{ 
              flex: 1, padding: "0.8rem", borderRadius: "8px", position: "relative",
              background: downloadComplete ? "rgba(16, 185, 129, 0.1)" : "rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)",
              border: downloadComplete ? "1px solid var(--tone-10b981, #10b981)" : "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)",
              color: downloadComplete ? "var(--tone-10b981, #10b981)" : "var(--tone-e2e8f0, #e2e8f0)",
              fontWeight: "bold", display: "flex", justifyContent: "center", alignItems: "center", gap: "0.5rem",
              overflow: "hidden"
            }}
            disabled={checkingVersion || isDownloading || loading || (downloadComplete && !versionStatus?.needs_update)}
            onClick={handleDownload}
          >
            {isDownloading && (
              <div className="wizard-download-fill" style={{
                position: "absolute", left: 0, top: 0, bottom: 0, width: `${progressPct}%`,
                background: "rgba(59, 130, 246, 0.3)", transition: "width 0.2s linear", zIndex: 0
              }} />
            )}
            <span style={{ zIndex: 1, pointerEvents: "none", display: "flex", alignItems: "center", gap: "0.5rem" }}>
              {checkingVersion ? t("setupWizard", "verifying") :
                isDownloading ? `${t("setupWizard", "downloading")} ${Math.round(progressPct)}% ${speedStr ? `(${speedStr})` : ""}` :
                downloadComplete && !versionStatus?.needs_update ? (
                  <>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
                    {t("setupWizard", "backendUpdated")} ({versionStatus?.current})
                  </>
                ) : (
                  <>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                    {t("setupWizard", "downloadGithub")} ({gpuArch === "rdna4" ? "RDNA 4" : "RDNA 3"})
                  </>
                )}
            </span>
          </button>
          
          {isDownloading ? (
            <button 
              className="btn" 
              onClick={handleCancelDownload}
              style={{ padding: "0.8rem 1.5rem", borderRadius: "8px", background: "rgba(239, 68, 68, 0.1)", border: "1px solid rgba(239, 68, 68, 0.3)", color: "var(--tone-f87171, #f87171)", fontWeight: "bold" }}
            >
              {t("setupWizard", "cancel")}
            </button>
          ) : (
            <button 
              className="btn" 
              onClick={handleLocalFolder}
              disabled={loading}
              title={t("setupWizard", "selectLocalFolderTitle")}
              style={{ flex: 1, padding: "0.8rem", borderRadius: "8px", background: "rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)", color: "var(--tone-e2e8f0, #e2e8f0)", display: "flex", justifyContent: "center", alignItems: "center", gap: "0.5rem", fontWeight: "bold" }}
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
              {t("setupWizard", "localFiles")}
            </button>
          )}
        </div>

        {backendPath && downloadComplete && !isDownloading && <button type="button" className="wizard-backend-path"
          title={backendPath} onClick={() => { void invoke("open_backend_folder", {gpuArch}).catch(error => setErrorMsg(String(error))); }}>{backendPath}</button>}
        <ModelSourceSelector 
          modelSource={modelSource}
          setModelSource={setModelSource}
          dllPath={dllPath}
          setDllPath={setDllPath}
          binPath={binPath}
          setBinPath={setBinPath}
          hasCachedBin={hasCachedBin}
          cachedPaths={cachedPaths}
          disabled={!gpuCompatible || !downloadComplete || isDownloading || loading || extractingDll || checkingVersion}
        />

        </fieldset>
      </ModalSurface>
    </div>
  );
};

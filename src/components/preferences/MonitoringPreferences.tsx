import { ComponentDownloadProgress } from "../ComponentDownloadProgress";
import { MenuIcon } from "../MenuIcon";
import { formatTransferBytes } from "../../services/downloadProgress";
import { ModernMangoHudEditor } from "./ModernMangoHudEditor";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useI18n } from "../../i18n/I18nContext";
import { invoke } from "@tauri-apps/api/core";
import { consumeMangoHudInstallationRequest, getMangoHudRelease, getMangoHudStatus, type MangoHudRelease, type MangoHudStatus, getDownloadedMangoHud, downloadMangoHud, installMangoHud, cancelMangoHudDownload, type MangoHudReady, type MangoHudProgress } from "../../services/mangohud";

export function MonitoringPreferences({ onDirtyChange, searchControl, searchQuery }: { onDirtyChange:(dirty:boolean)=>void; searchControl?: ReactNode; searchQuery?:string }) {
  const {t: tx} = useI18n();

  const [editorDirty,setEditorDirty]=useState(false);
  const [tab,setTab]=useState<"installation"|"monitoring"|"advanced">("monitoring");
  const [operation,setOperation]=useState<"download"|"install"|null>(null);
  const [ready,setReady]=useState<MangoHudReady|null>(null),[progress,setProgress]=useState<MangoHudProgress|null>(null);
  const [message,setMessage]=useState(""),[cancelling,setCancelling]=useState(false);
  const operationLock=useRef(false);
  const installationRef=useRef<HTMLButtonElement>(null);
  useEffect(()=>{onDirtyChange(editorDirty||Boolean(operation));return()=>onDirtyChange(false);},[editorDirty,operation,onDirtyChange]);
  const [status, setStatus] = useState<MangoHudStatus | null>(null);
  const [release, setRelease] = useState<MangoHudRelease | null>(null);
  const [checking, setChecking] = useState(false), [fetching, setFetching] = useState(false);
  const [error, setError] = useState("");
  const mounted = useRef(false), checkLock = useRef(false), fetchLock = useRef(false);
  // Installing runs this launcher as administrator; an AppImage that is not
  // installed is mounted where the administrator cannot read it.
  const [needsInstall, setNeedsInstall] = useState(false);
  useEffect(() => { if (!("__PELI_UI_PREVIEW__" in window)) void invoke<{ appimage: boolean }>("appimage_install_status").then(value => setNeedsInstall(value.appimage)).catch(() => {}); }, []);
  const check = async () => {
    if (checkLock.current) return; checkLock.current = true; setChecking(true); setError("");
    try { const result = await getMangoHudStatus(); if (mounted.current) setStatus(result); }
    catch (e) { if (mounted.current) { setStatus(null); setError(String(e)); } }
    finally { checkLock.current = false; if (mounted.current) setChecking(false); }
  };
  useEffect(() => { mounted.current = true; void check(); void getDownloadedMangoHud().then(r=>{if(mounted.current)setReady(r);}).catch(e=>{if(mounted.current)setError(String(e));}); return () => { mounted.current = false; }; }, []);
  const fetchRelease = async () => {
    if (fetchLock.current) return; fetchLock.current = true; setFetching(true); setError(""); setRelease(null);
    try { const result = await getMangoHudRelease(); if (mounted.current) setRelease(result); }
    catch (e) { if (mounted.current) setError(String(e)); }
    finally { fetchLock.current = false; if (mounted.current) setFetching(false); }
  };
  const run=async(kind:"download"|"install")=>{
    if(operationLock.current||kind==="download"&&!release)return;
    operationLock.current=true;setOperation(kind);setProgress(null);setError("");setMessage("");setCancelling(false);
    const report=(p:MangoHudProgress)=>{if(mounted.current)setProgress(p);};
    try{if(kind==="download"){const r=await downloadMangoHud(release!.version,report);if(mounted.current){setProgress(p=>({phase:"complete",downloaded:p?.downloaded??0,total:p?.total??0,speed_bytes_per_sec:0}));setReady(r);setMessage("Download concluído. O MangoHUD está pronto para instalar.");}}
      else{await installMangoHud(report);if(mounted.current){setProgress(p=>({phase:"complete",downloaded:p?.downloaded??0,total:p?.total??0,speed_bytes_per_sec:0}));setMessage("__PELI_UI_PREVIEW__" in window?"Instalação simulada concluída. Nenhum programa foi instalado no sistema.":"MangoHUD instalado com sucesso.");await check();}}}
    catch(e){if(mounted.current){setProgress(null);const detail=e instanceof Error?e.message:String(e);if(detail==="Download cancelado.")setMessage(detail);else setError(detail);}}
    finally{operationLock.current=false;if(mounted.current){setOperation(null);setCancelling(false);}}
  };
  const cancel=async()=>{setCancelling(true);try{await cancelMangoHudDownload();}catch(e){setError(String(e));setCancelling(false);}};
  const phase=progress?.phase??(operation==="install"?"authorizing":"checking");
  const phaseLabel=phase==="complete"?(tx("ui-monitoring-preferences", "complete")):phase==="downloading"?(tx("ui-monitoring-preferences", "downloading_mangohud")):phase==="verifying"?(tx("ui-monitoring-preferences", "verifying_integrity")):phase==="authorizing"?(tx("ui-monitoring-preferences", "waiting_for_administrator_authorization")):phase==="installing"?(tx("ui-monitoring-preferences", "installing_mangohud")):(tx("ui-monitoring-preferences", "preparing_download"));
  const showInstallation=()=>{setTab("installation");requestAnimationFrame(()=>installationRef.current?.focus());};
  useEffect(() => {
    const show = () => { if (consumeMangoHudInstallationRequest()) showInstallation(); };
    show(); window.addEventListener("showMangoHudInstallation", show);
    return () => window.removeEventListener("showMangoHudInstallation", show);
  }, []);
  const open = async (url: string) => { try { await openUrl(url); } catch (e) { if (mounted.current) setError(String(e)); } };
  const installation = <section className="mango-installation-view mod-workspace" aria-label={tx("ui-monitoring-preferences", "mangohud_installation")}>
    <header className="mod-intro"><h3>{tx("ui-monitoring-preferences", "prepare_mangohud_on_your_system")}</h3><p>{tx("ui-monitoring-preferences", "check_the_installation_download_the_package_and_install_it_then_customize_your_hud_in_th")}</p></header>
    <div className="mod-install-surface"><div className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="graphics"/></span><div><h4>MangoHUD {release?.version??ready?.version??""}</h4><p>{tx("ui-monitoring-preferences", "fps_and_hardware_information_in_your_games")}</p></div><span className="mod-status-badge" data-ready={status?.installed} role="status">{checking?(tx("ui-monitoring-preferences", "checking")):status?.installed?(tx("ui-monitoring-preferences", "detected")):status?(tx("ui-monitoring-preferences", "not_detected")):(tx("ui-monitoring-preferences", "not_checked"))}</span></div>
    {status?.executable&&<div className="mod-destination"><MenuIcon name="folder"/><div><span>{tx("ui-monitoring-preferences", "detected_executable")}</span><code>{status.executable}</code></div></div>}
    <div className="connection-primary-actions"><button type="button" className="mod-action-secondary" disabled={checking||Boolean(operation)} onClick={()=>void check()}><MenuIcon name="repair"/>{tx("ui-monitoring-preferences", "check_installation")}</button><button type="button" className="mod-action-secondary" disabled={fetching||Boolean(operation)} onClick={()=>void fetchRelease()}>{fetching?(tx("ui-monitoring-preferences", "checking_2")):(tx("ui-monitoring-preferences", "find_stable_release"))}</button></div>
    {(release||ready)&&<><div className="mod-setting-row"><div className="mod-row-copy"><strong>{ready&&(!release||ready.version===release.version)?(tx("ui-monitoring-preferences", "package_ready_to_install")):(tx("ui-monitoring-preferences", "download_official_package"))}</strong><small>{release?`${release.version} · ${formatTransferBytes(release.size)}`:ready?.version}</small></div>
      {operation==="download"?<button type="button" className="mod-action-secondary" disabled={cancelling} onClick={()=>void cancel()}>{tx("ui-monitoring-preferences", "cancel")}</button>:operation==="install"?<button type="button" className="mod-action-primary" disabled>{tx("ui-monitoring-preferences", "installing")}</button>:ready&&(!release||ready.version===release.version)?<button type="button" className="mod-action-primary" disabled={needsInstall} title={needsInstall?(tx("ui-monitoring-preferences", "install_peligames_on_your_system_to_install_mangohud")):undefined} onClick={()=>void run("install")}><MenuIcon name="download"/>{tx("ui-monitoring-preferences", "install_mangohud")}</button>:<button type="button" className="mod-action-primary" disabled={!release} onClick={()=>void run("download")}><MenuIcon name="download"/>{tx("ui-monitoring-preferences", "download_mangohud")}</button>}
    </div>{(operation||progress?.phase==="complete")&&<ComponentDownloadProgress onDismiss={()=>{setProgress(null);setMessage("")}} progress={{phase:phase==="checking"||phase==="authorizing"?"connecting":phase,downloaded:progress?.downloaded??0,total:progress?.total||null,speed_bytes_per_sec:progress?.speed_bytes_per_sec??0}} label={cancelling?(tx("ui-monitoring-preferences", "cancelling")):phaseLabel}/>}
    <p className="monitoring-install-note">{tx("ui-monitoring-preferences", "installation_uses_the_official_installer_and_requests_administrator_authorization_an_exi")}</p>
    {release&&<button type="button" className="mod-action-quiet" disabled={Boolean(operation)} onClick={()=>void open(release.release_url)}><MenuIcon name="externalLink"/>{tx("ui-monitoring-preferences", "release_notes")}</button>}
    </>}
    {needsInstall&&ready&&<p role="note">{tx("ui-monitoring-preferences", "first_install_peligames_on_the_system_from_the_welcome_screen_or_settings_tools")}</p>}
    {"__PELI_UI_PREVIEW__" in window&&<p className="monitoring-install-note">{tx("ui-monitoring-preferences", "demo_preview_nothing_is_installed_on_your_system")}</p>}
    {message&&<p role="status">{message}</p>}{error&&<p className="mod-feedback-error" role="alert">{error}</p>}
    </div>
  </section>;
  const searching=Boolean(searchQuery?.trim());
  return <div className="monitoring-workspace">
    <div className="monitoring-navigation" hidden={searching}><header className="settings-page-head"><div><h1>{tx("ui-monitoring-preferences", "monitoring")}</h1><p>{tx("ui-monitoring-preferences", "install_mangohud_and_customize_the_hud_for_your_games")}</p></div></header>
    <div className="settings-tab-toolbar"><div className="preference-mod-tabs monitoring-tabs" role="tablist" aria-label={"MangoHUD"} data-active={tab} onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key)||operation||editorDirty)return;event.preventDefault();const tabs=["installation","monitoring","advanced"] as const;const next=tabs[event.key==="Home"?0:event.key==="End"?tabs.length-1:(tabs.indexOf(tab)+(event.key==="ArrowRight"?1:tabs.length-1))%tabs.length];setTab(next);requestAnimationFrame(()=>document.getElementById(`mango-tab-${next}`)?.focus())}}>
      <button ref={installationRef} type="button" id="mango-tab-installation" role="tab" aria-selected={tab==="installation"} aria-controls="mango-panel-installation" tabIndex={tab==="installation"?0:-1} disabled={Boolean(operation)||editorDirty} onClick={()=>setTab("installation")}>{tx("ui-monitoring-preferences", "mangohud_installation")}</button>
      <button type="button" id="mango-tab-monitoring" role="tab" aria-selected={tab==="monitoring"} aria-controls="mango-panel-monitoring" tabIndex={tab==="monitoring"?0:-1} disabled={Boolean(operation)||editorDirty} onClick={()=>setTab("monitoring")}>{tx("ui-monitoring-preferences", "mangohud_monitoring")}</button>
      <button type="button" id="mango-tab-advanced" role="tab" aria-selected={tab==="advanced"} aria-controls="mango-panel-monitoring" tabIndex={tab==="advanced"?0:-1} disabled={Boolean(operation)||editorDirty} onClick={()=>setTab("advanced")}>{tx("ui-monitoring-preferences", "advanced_monitoring")}</button>
    </div>{searchControl}</div></div>
    <div className="monitoring-tab-panel" id="mango-panel-installation" role="tabpanel" aria-labelledby="mango-tab-installation" hidden={searching||tab!=="installation"}>{installation}</div>
    <div className="monitoring-tab-panel" id="mango-panel-monitoring" role={searching?undefined:"tabpanel"} aria-labelledby={searching?undefined:`mango-tab-${tab==="installation"?"monitoring":tab}`} hidden={!searching&&tab==="installation"}><ModernMangoHudEditor onDirtyChange={setEditorDirty} searchQuery={searchQuery} advancedOnly={tab==="advanced"} visible={!searching&&(tab==="monitoring"||tab==="advanced")}/></div>
  </div>;
}

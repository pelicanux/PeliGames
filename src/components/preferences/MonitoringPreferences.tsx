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
  const { language } = useI18n(); const pt = language === "pt";
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
  const phaseLabel=phase==="complete"?(pt?"Concluído":"Complete"):phase==="downloading"?(pt?"Baixando MangoHUD…":"Downloading MangoHUD…"):phase==="verifying"?(pt?"Verificando integridade…":"Verifying integrity…"):phase==="authorizing"?(pt?"Aguardando autorização de administrador…":"Waiting for administrator authorization…"):phase==="installing"?(pt?"Instalando MangoHUD…":"Installing MangoHUD…"):(pt?"Preparando download…":"Preparing download…");
  const showInstallation=()=>{setTab("installation");requestAnimationFrame(()=>installationRef.current?.focus());};
  useEffect(() => {
    const show = () => { if (consumeMangoHudInstallationRequest()) showInstallation(); };
    show(); window.addEventListener("showMangoHudInstallation", show);
    return () => window.removeEventListener("showMangoHudInstallation", show);
  }, []);
  const open = async (url: string) => { try { await openUrl(url); } catch (e) { if (mounted.current) setError(String(e)); } };
  const installation = <section className="mango-installation-view mod-workspace" aria-label={pt?"Instalação do MangoHUD":"MangoHUD installation"}>
    <header className="mod-intro"><h3>{pt?"Prepare o MangoHUD no sistema":"Prepare MangoHUD on your system"}</h3><p>{pt?"Verifique a instalação, baixe o pacote e instale. Depois personalize o painel na aba Monitoramento.":"Check the installation, download the package and install it. Then customize your HUD in the Monitoring tab."}</p></header>
    <div className="mod-install-surface"><div className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="graphics"/></span><div><h4>MangoHUD {release?.version??ready?.version??""}</h4><p>{pt?"FPS e informações de hardware nos seus jogos.":"FPS and hardware information in your games."}</p></div><span className="mod-status-badge" data-ready={status?.installed} role="status">{checking?(pt?"Verificando…":"Checking…"):status?.installed?(pt?"Detectado":"Detected"):status?(pt?"Não detectado":"Not detected"):(pt?"Não verificado":"Not checked")}</span></div>
    {status?.executable&&<div className="mod-destination"><MenuIcon name="folder"/><div><span>{pt?"Executável detectado":"Detected executable"}</span><code>{status.executable}</code></div></div>}
    <div className="connection-primary-actions"><button type="button" className="mod-action-secondary" disabled={checking||Boolean(operation)} onClick={()=>void check()}><MenuIcon name="repair"/>{pt?"Verificar instalação":"Check installation"}</button><button type="button" className="mod-action-secondary" disabled={fetching||Boolean(operation)} onClick={()=>void fetchRelease()}>{fetching?(pt?"Buscando…":"Checking…"):(pt?"Buscar versão estável":"Find stable release")}</button></div>
    {(release||ready)&&<><div className="mod-setting-row"><div className="mod-row-copy"><strong>{ready&&(!release||ready.version===release.version)?(pt?"Pacote pronto para instalar":"Package ready to install"):(pt?"Baixar pacote oficial":"Download official package")}</strong><small>{release?`${release.version} · ${formatTransferBytes(release.size)}`:ready?.version}</small></div>
      {operation==="download"?<button type="button" className="mod-action-secondary" disabled={cancelling} onClick={()=>void cancel()}>{pt?"Cancelar":"Cancel"}</button>:operation==="install"?<button type="button" className="mod-action-primary" disabled>{pt?"Instalando…":"Installing…"}</button>:ready&&(!release||ready.version===release.version)?<button type="button" className="mod-action-primary" disabled={needsInstall} title={needsInstall?(pt?"Instale o PeliGames no sistema para instalar o MangoHUD.":"Install PeliGames on your system to install MangoHUD."):undefined} onClick={()=>void run("install")}><MenuIcon name="download"/>{pt?"Instalar MangoHUD":"Install MangoHUD"}</button>:<button type="button" className="mod-action-primary" disabled={!release} onClick={()=>void run("download")}><MenuIcon name="download"/>{pt?"Baixar MangoHUD":"Download MangoHUD"}</button>}
    </div>{(operation||progress?.phase==="complete")&&<ComponentDownloadProgress onDismiss={()=>{setProgress(null);setMessage("")}} progress={{phase:phase==="checking"||phase==="authorizing"?"connecting":phase,downloaded:progress?.downloaded??0,total:progress?.total||null,speed_bytes_per_sec:progress?.speed_bytes_per_sec??0}} label={cancelling?(pt?"Cancelando…":"Cancelling…"):phaseLabel}/>}
    <p className="monitoring-install-note">{pt?"A instalação usa o instalador oficial e pede autorização de administrador. Uma instalação existente pode ser atualizada.":"Installation uses the official installer and requests administrator authorization. An existing installation may be updated."}</p>
    {release&&<button type="button" className="mod-action-quiet" disabled={Boolean(operation)} onClick={()=>void open(release.release_url)}><MenuIcon name="externalLink"/>{pt?"Notas da versão":"Release notes"}</button>}
    </>}
    {needsInstall&&ready&&<p role="note">{pt?"Instale primeiro o PeliGames no sistema pela tela de boas-vindas ou em Configurações → Ferramentas.":"First install PeliGames on the system from the welcome screen or Settings → Tools."}</p>}
    {"__PELI_UI_PREVIEW__" in window&&<p className="monitoring-install-note">{pt?"Prévia demonstrativa: nenhuma instalação é feita no sistema.":"Demo preview: nothing is installed on your system."}</p>}
    {message&&<p role="status">{message}</p>}{error&&<p className="mod-feedback-error" role="alert">{error}</p>}
    </div>
  </section>;
  const searching=Boolean(searchQuery?.trim());
  return <div className="monitoring-workspace">
    <div className="monitoring-navigation" hidden={searching}><header className="settings-page-head"><div><h1>{pt?"Monitoramento":"Monitoring"}</h1><p>{pt?"Instale o MangoHUD e personalize o painel dos seus jogos.":"Install MangoHUD and customize the HUD for your games."}</p></div></header>
    <div className="settings-tab-toolbar"><div className="preference-mod-tabs monitoring-tabs" role="tablist" aria-label={pt?"MangoHUD":"MangoHUD"} data-active={tab} onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key)||operation||editorDirty)return;event.preventDefault();const tabs=["installation","monitoring","advanced"] as const;const next=tabs[event.key==="Home"?0:event.key==="End"?tabs.length-1:(tabs.indexOf(tab)+(event.key==="ArrowRight"?1:tabs.length-1))%tabs.length];setTab(next);requestAnimationFrame(()=>document.getElementById(`mango-tab-${next}`)?.focus())}}>
      <button ref={installationRef} type="button" id="mango-tab-installation" role="tab" aria-selected={tab==="installation"} aria-controls="mango-panel-installation" tabIndex={tab==="installation"?0:-1} disabled={Boolean(operation)||editorDirty} onClick={()=>setTab("installation")}>{pt?"Instalação do MangoHUD":"MangoHUD installation"}</button>
      <button type="button" id="mango-tab-monitoring" role="tab" aria-selected={tab==="monitoring"} aria-controls="mango-panel-monitoring" tabIndex={tab==="monitoring"?0:-1} disabled={Boolean(operation)||editorDirty} onClick={()=>setTab("monitoring")}>{pt?"Monitoramento do MangoHUD":"MangoHUD monitoring"}</button>
      <button type="button" id="mango-tab-advanced" role="tab" aria-selected={tab==="advanced"} aria-controls="mango-panel-monitoring" tabIndex={tab==="advanced"?0:-1} disabled={Boolean(operation)||editorDirty} onClick={()=>setTab("advanced")}>{pt?"Monitoramento avançado":"Advanced monitoring"}</button>
    </div>{searchControl}</div></div>
    <div className="monitoring-tab-panel" id="mango-panel-installation" role="tabpanel" aria-labelledby="mango-tab-installation" hidden={searching||tab!=="installation"}>{installation}</div>
    <div className="monitoring-tab-panel" id="mango-panel-monitoring" role={searching?undefined:"tabpanel"} aria-labelledby={searching?undefined:`mango-tab-${tab==="installation"?"monitoring":tab}`} hidden={!searching&&tab==="installation"}><ModernMangoHudEditor onDirtyChange={setEditorDirty} searchQuery={searchQuery} advancedOnly={tab==="advanced"} visible={!searching&&(tab==="monitoring"||tab==="advanced")}/></div>
  </div>;
}

import { ModernMangoHudEditor } from "./ModernMangoHudEditor";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useI18n } from "../../i18n/I18nContext";
import { invoke } from "@tauri-apps/api/core";
import { consumeMangoHudInstallationRequest, getMangoHudRelease, getMangoHudStatus, type MangoHudRelease, type MangoHudStatus, getDownloadedMangoHud, downloadMangoHud, installMangoHud, cancelMangoHudDownload, type MangoHudReady, type MangoHudProgress } from "../../services/mangohud";

export function MonitoringPreferences({ onDirtyChange, searchControl }: { onDirtyChange:(dirty:boolean)=>void; searchControl?: ReactNode }) {
  const { language } = useI18n(); const pt = language === "pt";
  const [editorDirty,setEditorDirty]=useState(false);
  const [operation,setOperation]=useState<"download"|"install"|null>(null);
  const [ready,setReady]=useState<MangoHudReady|null>(null),[progress,setProgress]=useState<MangoHudProgress|null>(null);
  const [message,setMessage]=useState(""),[cancelling,setCancelling]=useState(false);
  const operationLock=useRef(false);
  const installationRef=useRef<HTMLDetailsElement>(null);
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
    try{if(kind==="download"){const r=await downloadMangoHud(release!.version,report);if(mounted.current){setReady(r);setMessage("Download concluído. O MangoHUD está pronto para instalar.");}}
      else{await installMangoHud(report);if(mounted.current){setMessage("__PELI_UI_PREVIEW__" in window?"Instalação simulada concluída. Nenhum programa foi instalado no sistema.":"MangoHUD instalado com sucesso.");await check();}}}
    catch(e){if(mounted.current){const detail=e instanceof Error?e.message:String(e);if(detail==="Download cancelado.")setMessage(detail);else setError(detail);}}
    finally{operationLock.current=false;if(mounted.current){setOperation(null);setCancelling(false);}}
  };
  const cancel=async()=>{setCancelling(true);try{await cancelMangoHudDownload();}catch(e){setError(String(e));setCancelling(false);}};
  const percent=progress?.total?Math.min(100,progress.downloaded/progress.total*100):0;
  const phase=progress?.phase??(operation==="install"?"authorizing":"checking");
  const phaseLabel=phase==="downloading"?`Baixando… ${percent.toFixed(0)}% · ${((progress?.speed_bytes_per_sec??0)/1048576).toFixed(1)} MB/s`:phase==="verifying"?"Verificando integridade…":phase==="authorizing"?"Aguardando autorização de administrador…":phase==="installing"?"Instalando MangoHUD…":"Preparando download…";
  const showInstallation=()=>{const panel=installationRef.current;if(panel){panel.open=true;panel.scrollIntoView({behavior:"smooth",block:"start"});}};
  useEffect(() => {
    const show = () => { if (consumeMangoHudInstallationRequest()) showInstallation(); };
    show(); window.addEventListener("showMangoHudInstallation", show);
    return () => window.removeEventListener("showMangoHudInstallation", show);
  }, []);
  const open = async (url: string) => { try { await openUrl(url); } catch (e) { if (mounted.current) setError(String(e)); } };
  const installation = <details ref={installationRef} className="preference-panel monitoring-panel mango-installation" open>
      <summary>Instalação e versão do MangoHUD · {checking ? "Verificando…" : status?.installed ? "Detectado" : "Não detectado"}</summary>
      <h3>{pt ? "Instalação no sistema" : "System installation"}</h3>
      <p role="status">{checking ? (pt ? "Verificando MangoHUD…" : "Checking MangoHUD…") : status ? (status.installed ? (pt ? "MangoHUD detectado no sistema." : "MangoHUD detected on your system.") : (pt ? "MangoHUD não foi detectado no sistema." : "MangoHUD was not detected on your system.")) : (pt ? "Não foi possível verificar a instalação." : "Could not check the installation.")}</p>
      {status?.executable && <p><code>{status.executable}</code></p>}
      <div className="monitoring-actions"><button type="button" disabled={checking||Boolean(operation)} onClick={() => void check()}>{pt ? "Verificar novamente" : "Check again"}</button><button type="button" disabled={fetching||Boolean(operation)} onClick={() => void fetchRelease()}>{fetching ? (pt ? "Buscando versão estável…" : "Looking up stable release…") : (pt ? "Buscar última versão estável" : "Find latest stable release")}</button></div>
      {(release||ready)&&<div className="monitoring-release">
        <div className={`proton-distribution-card mango-install-card ${operation?"is-downloading":""}`}>
          {operation&&<span className={`proton-card-progress ${operation==="install"?"mango-install-working":""}`} style={{transform:`scaleX(${phase==="downloading"?percent/100:1})`}}/>}
          <div className="proton-distribution-info"><span><strong>MangoHUD {release?.version??ready?.version}</strong><small role="status">{operation?(cancelling?"Cancelando download…":phaseLabel):ready&&(!release||ready.version===release.version)?"Pacote baixado · pronto para instalar":`${((release?.size??0)/1048576).toFixed(1)} MiB · versão estável oficial`}</small></span></div>
          <div className="monitoring-actions mango-install-actions">
            {operation==="download"?<button type="button" disabled={cancelling} onClick={()=>void cancel()}>Cancelar</button>:operation==="install"?<button type="button" disabled>Instalando…</button>:ready&&(!release||ready.version===release.version)?<button type="button" disabled={needsInstall} title={needsInstall ? (pt ? "Instale o PeliGames no sistema (botão Instalar) para instalar o MangoHUD." : "Install PeliGames (Install button) to install MangoHUD.") : undefined} onClick={()=>void run("install")}>Instalar MangoHUD</button>:<button type="button" disabled={!release} onClick={()=>void run("download")}>Baixar MangoHUD</button>}
          </div>
        </div>
        <p>A instalação executa o instalador oficial e solicita autorização de administrador. Ela pode atualizar uma instalação existente do MangoHUD.</p>
        {"__PELI_UI_PREVIEW__" in window&&<p>Pré-visualização: download e instalação demonstrativos, sem alterar o sistema.</p>}
        {release&&<div className="monitoring-actions"><button type="button" disabled={Boolean(operation)} onClick={()=>void open(release.release_url)}>Notas da versão ↗</button></div>}
      </div>}
      {needsInstall&&ready&&<p role="note">{pt ? "Para instalar o MangoHUD, instale primeiro o PeliGames no sistema pelo botão Instalar da tela de boas-vindas ou em Configurações → Ferramentas." : "To install MangoHUD, first install PeliGames on the system with the Install button on the welcome screen or in Settings → Tools."}</p>}
      {message&&<p role="status">{message}</p>}
      {error && <p className="preferences-error" role="alert">{error}</p>}
    </details>;
  return <ModernMangoHudEditor searchControl={searchControl} onDirtyChange={setEditorDirty} installation={installation}/>;
}

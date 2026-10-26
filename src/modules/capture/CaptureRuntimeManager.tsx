import {MenuIcon} from "../../components/MenuIcon";
import {ConfirmModal} from "../../components/ConfirmModal";
import {ComponentDownloadProgress} from "../../components/ComponentDownloadProgress";
import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
type Runtime={installed:boolean;version:string;source:string;bytes:number;directory:string};
type Progress={phase:string;downloaded:number;total:number|null};
export function CaptureRuntimeManager({locked,onChanged,onBusy}:{locked:boolean;onChanged?:()=>Promise<void>;onBusy?:(busy:boolean)=>void}) {
 const [runtime,setRuntime]=useState<Runtime>(),[busy,setBusy]=useState(false),[error,setError]=useState(""),[progress,setProgress]=useState<Progress>(),[confirm,setConfirm]=useState(false);
 useEffect(()=>{let active=true;let off:(()=>void)|undefined;const update=(p:Progress)=>{if(active)setProgress(p)};const demo=(e:Event)=>update((e as CustomEvent<Progress>).detail);
 window.addEventListener("capture-runtime-progress-preview",demo);
 void(async()=>{try{off=await listen<Progress>("capture-runtime-progress",e=>update(e.payload));if(!active){off();return;}const value=await invoke<Runtime>("capture_runtime_status");if(active)setRuntime(value);}catch(e){if(active)setError(String(e));}})();
 return()=>{active=false;off?.();window.removeEventListener("capture-runtime-progress-preview",demo);};},[]);
 useEffect(()=>{onBusy?.(busy);return()=>onBusy?.(false);},[busy,onBusy]);
 const run=async(remove:boolean)=>{setBusy(true);setError("");setConfirm(false);setProgress(undefined);try{setRuntime(await invoke<Runtime>(remove?"capture_runtime_remove":"capture_runtime_install"));await onChanged?.();window.dispatchEvent(new Event("capture-runtime-changed"));}catch(e){setError(String(e));}finally{setBusy(false);}};
 const phase=progress?.phase==="extracting"?"Preparando arquivos…":progress?.phase==="verifying"?"Verificando integridade…":progress?.phase==="installed"?"Componente instalado":"Baixando componente…";
 return <>
  <section className="mod-install-surface" aria-label="Instalação do GPU Screen Recorder">
    <div className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="platform"/></span><div><h4>GPU Screen Recorder</h4><p>Uma instalação compartilhada entre os jogos.</p></div><span className="mod-status-badge" data-ready={runtime?.installed||false} role="status">{!runtime?"Verificando…":runtime.installed?`Instalado · ${runtime.version}`:"Não instalado"}</span></div>
    <div className="mod-step-actions"><button type="button" className={runtime?.installed?"mod-action-secondary":"mod-action-primary"} disabled={locked||busy||!runtime} onClick={()=>void run(false)}><MenuIcon name={runtime?.installed?"repair":"download"}/>{busy?phase:runtime?.installed?"Reparar componente":"Instalar componente de captura"}</button></div>
    {busy&&progress&&<ComponentDownloadProgress progress={{...progress,phase:progress.phase==="installed"?"complete":progress.phase}} label={phase}/>}
    {error&&<p role="alert" className="mod-feedback-error">{error}</p>}
    <div className="mod-destination"><MenuIcon name="folder"/><div><span>Destino do componente</span><button type="button" disabled={!runtime?.installed||busy} title={runtime?.directory} onClick={()=>{if("__PELI_UI_PREVIEW__" in window)return;void invoke("open_folder",{path:runtime?.directory}).catch(e=>setError(String(e)));}}>{runtime?.directory||"Verificando destino…"}</button><small className="mod-version-label">{runtime?.installed?`Versão ${runtime.version} · ${(runtime.bytes/1048576).toFixed(0)} MB instalados`:`Versão compatível: ${runtime?.version||"verificando…"}`} · pacote portátil de pkgforge-dev.</small></div></div>
  </section>
  <section className="mod-next-step"><header className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="gamepad"/></span><div><h4>Configure a captura de cada jogo</h4><p>Abra os detalhes do jogo → Capturas para escolher fonte, áudio e replay.</p></div></header><div className="mod-requirement"><MenuIcon name="info"/><p>Compatível com AMD, Intel e NVIDIA (NVENC). Os codecs dependem da GPU e do driver. A instalação não inicia gravações.</p></div></section>
  {runtime?.installed&&<div className="mod-backend-tools"><div className="mod-row-copy"><h4>Remover componente</h4><p>Remove apenas o backend do PeliGames. Vídeos, configurações e o gravador do sistema são preservados.</p></div><div className="mod-tool-actions"><button type="button" className="mod-action-quiet danger" disabled={locked||busy} onClick={()=>setConfirm(true)}><MenuIcon name="trash"/>Remover componente</button></div></div>}
  {confirm&&<ConfirmModal title="Remover GPU Screen Recorder?" message="Remover somente o componente instalado pelo PeliGames? Seus vídeos e configurações serão mantidos." confirmText="Confirmar remoção" isDanger onConfirm={()=>void run(true)} onCancel={()=>setConfirm(false)}/>}
 </>;
}

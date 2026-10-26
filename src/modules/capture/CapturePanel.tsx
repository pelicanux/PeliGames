import {CaptureShortcutKeys} from "./CaptureShortcutKeys";
import {ReplayDurationField} from "./ReplayDurationField";
import {CaptureVideoLibrary,type CaptureClip} from "./CaptureVideoLibrary";
import {CaptureQualityPresets} from "./CaptureQualityPresets";
import {CaptureShortcuts} from "./CaptureShortcuts";
import {CaptureAudioFields,type AudioSelection} from "./CaptureAudioFields";
import {useEffect, useId, useRef, useState} from "react";
import {open as chooseFolder} from "@tauri-apps/plugin-dialog";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {openCapturePreferences} from "../../services/frameGenerationNavigation";
import {SelectionMenu} from "../../components/SelectionMenu";
import "./capturePreview.css";

type Status={phase:string;game_key:string|null;recording:boolean;replay:boolean;last_file:string|null;error:string|null};
type Support={installed:boolean;ipc:boolean;portal:boolean;monitors:string[];executable:string|null};
type Clip=CaptureClip;
type StoragePreferences={directory:string;default_directory:string};
type CaptureTab="settings"|"capture"|"shortcuts"|"videos";
const captureTabs:{id:CaptureTab;label:string}[]=[{id:"settings",label:"Configurações"},{id:"capture",label:"Gravação e replay"},{id:"shortcuts",label:"Atalhos"},{id:"videos",label:"Vídeos"}];
type Settings=AudioSelection&{method:string;monitor:string;replay_seconds:number};
const defaults:Settings={method:"portal",monitor:"",replay_seconds:60,fps:60,desktop_audio:true,microphone:false,microphone_noise_reduction:false,format:"mkv",codec:"auto",resolution:"1280x720",rate_control:"cbr",bitrate_kbps:5000,quality_preset:"very_high",output_source:"game:auto",input_source:"default_input"};
function loadSettings(key:string):Settings {
  try {const v=JSON.parse(localStorage.getItem(`peligames-capture:${key}`)||"null");return {...defaults,...v,method:v?.method==="direct"?"direct":"portal",replay_seconds:Number.isInteger(v?.replay_seconds)&&v.replay_seconds>=1&&v.replay_seconds<=900?v.replay_seconds:60,fps:[24,30,60,90,120,144,165,240].includes(v?.fps)?v.fps:60};}catch{return defaults;}
}
export function CapturePanel({gameKey,gameName,gameDirectory="",gameExecutable="",steamAppId=""}:{gameKey:string;gameName:string;gameDirectory?:string;gameExecutable?:string;steamAppId?:string}) {
  const tabsId=useId();
  const [tab,setTab]=useState<CaptureTab>("settings");
  const [customPreset,setCustomPreset]=useState(false);
  const [shortcuts,setShortcuts]=useState<{recording:string;replay:string;save_replay:string}>();
  const [settings,setSettings]=useState(()=>loadSettings(gameKey));
  const [storage,setStorage]=useState<StoragePreferences>();
  const [storageBusy,setStorageBusy]=useState(false);
  const [support,setSupport]=useState<Support>();
  const [status,setStatus]=useState<Status>({phase:"idle",game_key:null,recording:false,replay:false,last_file:null,error:null});
  const [clips,setClips]=useState<Clip[]>([]),[folder,setFolder]=useState("");
  const [overlaySyncError,setOverlaySyncError]=useState("");
  const [selectionLoaded,setSelectionLoaded]=useState(false);
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[notice,setNotice]=useState("");
  const alive=useRef(true);
  const operation=useRef(0);
  const preview="__PELI_UI_PREVIEW__" in window;
  useEffect(()=>{let mounted=true;alive.current=true;let unlisten:(()=>void)|undefined;
    const update=(value:Status)=>{if(mounted){setStatus(value);void invoke<Clip[]>("capture_clips",{gameKey,gameName}).then(items=>{if(mounted)setClips(items);}).catch(e=>{if(mounted)setError(String(e));});}};
    const previewEvent=(event:Event)=>update((event as CustomEvent<Status>).detail);
    const runtimeChanged=()=>{void invoke<Support>("capture_support").then(value=>{if(mounted){setSupport(value);}}).catch(e=>{if(mounted)setError(String(e));});};
    window.addEventListener("capture-runtime-changed",runtimeChanged);
    if(preview)window.addEventListener("peligames-capture-preview",previewEvent);
    void (async()=>{try {
      unlisten=await listen<Status>("peligames-capture",event=>update(event.payload));
      if(!mounted){unlisten();return;}
      update(await invoke<Status>("capture_status"));
      const [detected,path]=await Promise.all([invoke<Support>("capture_support"),invoke<string>("capture_folder",{gameKey,gameName})]);
      if(mounted){setSupport(detected);setFolder(path);}
    }catch(e){if(mounted)setError(String(e));}})();
    return()=>{mounted=false;alive.current=false;window.removeEventListener("capture-runtime-changed",runtimeChanged);unlisten?.();window.removeEventListener("peligames-capture-preview",previewEvent);};
  },[gameKey,gameName]);
  useEffect(()=>{let mounted=true;let off:(()=>void)|undefined;
    setSelectionLoaded(false);setSettings(loadSettings(gameKey));
    void invoke<Partial<Settings>|null>("capture_settings",{gameKey}).then(selection=>{if(mounted){if(selection)setSettings(old=>({...old,...selection}));setSelectionLoaded(true);}}).catch(e=>{if(mounted)setError(`Não foi possível carregar os ajustes de captura: ${String(e)}`);});
    void listen<{game_key:string;selection:AudioSelection}>("capture-selection-changed",event=>{if(mounted&&event.payload.game_key===gameKey){setSettings(old=>{const value={...old,...event.payload.selection};try{localStorage.setItem(`peligames-capture:${gameKey}`,JSON.stringify(value));}catch{}return value;});}}).then(unlisten=>{if(mounted)off=unlisten;else unlisten();}).catch(()=>{});
    return()=>{mounted=false;off?.();};
  },[gameKey]);
  useEffect(()=>{let mounted=true;let off:(()=>void)|undefined;
    const refresh=()=>{void Promise.all([invoke<StoragePreferences>("capture_storage_preferences"),invoke<string>("capture_folder",{gameKey,gameName}),invoke<Clip[]>("capture_clips",{gameKey,gameName})]).then(([value,path,items])=>{if(mounted){setStorage(value);setFolder(path);setClips(items);}}).catch(e=>{if(mounted)setError(String(e));});};
    refresh();void listen("capture-storage-changed",refresh).then(unlisten=>{if(mounted)off=unlisten;else unlisten();}).catch(()=>{});
    const onPreview=()=>refresh();window.addEventListener("capture-storage-preview",onPreview);
    return()=>{mounted=false;off?.();window.removeEventListener("capture-storage-preview",onPreview);};
  },[gameKey,gameName]);
  const changeStorage=async(reset=false)=>{
    setStorageBusy(true);setError("");
    try {
      const directory=reset?null:await chooseFolder({directory:true,multiple:false,title:"Onde salvar gravações e clipes?",defaultPath:storage?.directory});
      if(!reset&&typeof directory!=="string")return;
      const value=await invoke<StoragePreferences>("configure_capture_storage",{directory});
      const [path,items]=await Promise.all([invoke<string>("capture_folder",{gameKey,gameName}),invoke<Clip[]>("capture_clips",{gameKey,gameName})]);
      if(alive.current){setStorage(value);setFolder(path);setClips(items);setNotice("Pasta de gravações atualizada. Os arquivos anteriores permanecem no local original.");}
    }catch(e){if(alive.current)setError(String(e));}finally{if(alive.current)setStorageBusy(false);}
  };
  useEffect(()=>{if(tab!=="capture")return;let mounted=true;void invoke<{recording:string;replay:string;save_replay:string}>("capture_shortcut_preferences").then(value=>{if(mounted)setShortcuts(value);}).catch(()=>{});return()=>{mounted=false;};},[tab]);
  const change=(next:Partial<Settings>)=>{if(["resolution","fps","rate_control","bitrate_kbps","codec","quality_preset"].some(key=>key in next))setCustomPreset(true);const value={...settings,...next};setSettings(value);try{localStorage.setItem(`peligames-capture:${gameKey}`,JSON.stringify(value));}catch{setError("Não foi possível salvar as preferências de captura.");}};
  const active=status.phase==="active",starting=status.phase==="starting",own=status.game_key===gameKey;
  const locked=active||starting||busy||storageBusy;
  const monitor=settings.monitor||support?.monitors[0]||"";
  const ready=!storageBusy&&support?.installed&&support.ipc&&(settings.method==="portal"?support.portal:support.monitors.includes(monitor));
  const run=async(task:()=>Promise<Status>,message:string)=>{const request=++operation.current;setBusy(true);setError("");setNotice("");try{const next=await task();if(alive.current&&request===operation.current){setStatus(next);setNotice(message);setClips(await invoke<Clip[]>("capture_clips",{gameKey,gameName}));}}catch(e){if(alive.current&&request===operation.current)setError(String(e));}finally{if(alive.current&&request===operation.current)setBusy(false);}};

  useEffect(()=>{
    if(!selectionLoaded||active||starting||busy)return;
    let mounted=true;
    const timer=window.setTimeout(()=>{void invoke("capture_prepare_overlay",{options:{...settings,monitor,game_key:gameKey,game_name:gameName,game_directory:gameDirectory,game_executable:gameExecutable,steam_app_id:steamAppId}}).then(()=>{if(mounted)setOverlaySyncError("");}).catch(e=>{if(mounted)setOverlaySyncError(`Não foi possível vincular os controles de captura ao jogo: ${String(e)}`);});},200);
    return()=>{mounted=false;window.clearTimeout(timer);};
  },[selectionLoaded,settings,monitor,gameKey,gameName,gameDirectory,gameExecutable,steamAppId,active,starting,busy]);
  const start=(replay:boolean)=>void run(()=>invoke<Status>("capture_start",{options:{...settings,monitor,game_key:gameKey,game_name:gameName,game_directory:gameDirectory,game_executable:gameExecutable,steam_app_id:steamAppId},replay}),preview?"Simulação iniciada. Nenhuma tela ou áudio foi capturado.":replay?"Replay pronto para salvar.":"Gravação iniciada.");
  const action=(action:string)=>void run(()=>invoke<Status>("capture_action",{action}),action==="save-replay"?"Replay salvo.":action==="stop"?"Captura encerrada.":action==="stop-recording"?"Gravação salva.":"Gravação iniciada.");
  const open=(path:string)=>{if(preview){setNotice("Prévia: nenhum arquivo real é aberto.");return;}void invoke("open_folder",{path}).catch(e=>setError(String(e)));};
  return <div className="capture-workspace">
    <div className="settings-tab-toolbar capture-tab-toolbar"><div className="preference-mod-tabs capture-tabs" role="tablist" aria-label="Opções de captura" data-active={tab}
      onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();const index=captureTabs.findIndex(t=>t.id===tab);const next=captureTabs[event.key==="Home"?0:event.key==="End"?captureTabs.length-1:(index+(event.key==="ArrowRight"?1:-1)+captureTabs.length)%captureTabs.length].id;setTab(next);document.getElementById(`${tabsId}-tab-${next}`)?.focus();}}>
      {captureTabs.map(t=><button key={t.id} id={`${tabsId}-tab-${t.id}`} type="button" role="tab" aria-selected={tab===t.id} aria-controls={`${tabsId}-panel-${t.id}`} tabIndex={tab===t.id?0:-1} onClick={()=>setTab(t.id)}>{t.label}</button>)}
    </div>{(active||starting)&&<span className="capture-tab-status" role="status">{starting?"Preparando…":status.recording?"Gravando":"Replay ativo"}{active&&!own?" · outro jogo":""}</span>}</div>
    <div className="capture-preview" role="tabpanel" id={`${tabsId}-panel-${tab}`} aria-labelledby={`${tabsId}-tab-${tab}`}>
    {tab==="capture"&&<section className="modern-performance-card capture-preview-main">
      <div className="modern-panel-heading"><h3>Capturar gameplay</h3><span className="capture-demo-label">{preview?"Simulação":"GPU Screen Recorder"}</span></div>
      <p>Grave uma sessão ou guarde os últimos momentos de {gameName}.</p>
      <div className="capture-preview-status" role="status"><i data-active={active||starting}/>{starting?"Aguardando seleção, autorização ou inicialização…":active?`${status.recording?"Gravando":"Replay ativo"}${status.replay&&status.recording?" · replay ativo":""}${own?"":" · outro jogo"}`:"Captura desligada"}</div>
      <div className="capture-settings-columns capture-recording-modes"><div>
        <div className="capture-mode-heading"><h4>Gravação de sessão</h4><CaptureShortcutKeys value={shortcuts?.recording}/></div>
        <p>Comece a gravar e escolha quando parar. A sessão é salva inteira, sem depender da duração do replay.</p>
        <div className="capture-preview-actions capture-session-actions">
          <button type="button" className="modern-mini-btn capture-primary" disabled={busy||starting||(!active&&!ready)||(active&&!own)} onClick={()=>active?action(status.recording?(status.replay?"stop-recording":"stop"):"start-recording"):start(false)}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 16 16" fill="currentColor">{status.recording?<rect x="4" y="4" width="8" height="8" rx="1.5"/>:<circle cx="8" cy="8" r="4"/>}</svg>{status.recording?"Parar e salvar gravação":"Gravar sessão"}</button>
          {(active||starting)&&<button type="button" className="modern-mini-btn" disabled={busy&&!starting} onClick={()=>action("stop")}>{starting?"Cancelar captura":"Encerrar captura"}</button>}
        </div>
        <p>Usa a qualidade escolhida em Configurações. Você também pode gravar uma sessão enquanto o replay está ativo.</p>
      </div><div>
        <div className="capture-mode-heading"><h4>Replay instantâneo</h4></div>
        <p>Ative antes de jogar. Ao salvar, guarda o trecho que já passou, até a duração escolhida, e continua mantendo o replay ativo.</p>
        <div className="capture-replay-activation"><div><strong>{starting&&own&&status.replay?"Preparando replay…":"Manter replay ativo"}</strong><CaptureShortcutKeys value={shortcuts?.replay}/></div><button type="button" role="switch" aria-label="Replay instantâneo" aria-checked={(active||starting)&&status.replay} disabled={starting? !own||!status.replay : busy||(!active&&!ready)||(active&&(!own||!status.replay))} className={`neural-startup-switch mod-option-switch${(active||starting)&&status.replay?" enabled":""}`} onClick={()=>starting||active?action("stop"):start(true)}><span className="neural-switch-track"><span/></span></button></div>
        {starting&&own&&status.replay&&<p>Clique novamente no controle para cancelar a preparação.</p>}
        <div className="capture-replay-toolbar"><button type="button" className="modern-mini-btn capture-replay-save" disabled={busy||!active||!status.replay||!own} onClick={()=>action("save-replay")}><CaptureShortcutKeys value={shortcuts?.save_replay}/><span>Salvar replay</span></button><ReplayDurationField compact seconds={settings.replay_seconds} disabled={locked} onChange={replay_seconds=>change({replay_seconds})}/></div>
        <p className="capture-replay-estimate">{settings.rate_control==="cbr"?`${settings.bitrate_kbps/1000} Mb/s · cerca de ${Math.round(settings.replay_seconds*settings.bitrate_kbps/8000)} MB de vídeo no replay, além do áudio e do uso do codificador.`:"O tamanho do replay varia com a qualidade e o conteúdo capturado, além do áudio e do uso do codificador."}</p>
      </div></div>
      <div className="capture-overlay-footer"><p>Os controles de captura ficam no Overlay PeliGames, junto do MAKO. Ative-o em Desempenho e use os atalhos configurados. As escolhas desta aba são vinculadas automaticamente ao jogo, sem iniciar uma gravação.</p>
      {overlaySyncError&&<p role="alert" className="capture-error">{overlaySyncError}</p>}
      {!support&&!error&&<p>Verificando o gravador instalado…</p>}
      {support&&!support.installed&&<p role="alert">Instale o GPU Screen Recorder em Configurações → Complementos → GPU Screen Recorder. Se já houver um backend nativo compatível no sistema, o PeliGames também pode utilizá-lo.</p>}
      {support?.installed&&!support.ipc&&<p role="alert">Atualize o GPU Screen Recorder: esta versão precisa oferecer controle por IPC.</p>}
      </div>
    </section>}
    {tab==="settings"&&<section className="modern-performance-card capture-preview-settings"><h3>Configurações de gravação</h3>
      <CaptureQualityPresets settings={settings} disabled={locked} custom={customPreset} onCustom={()=>setCustomPreset(true)} onChange={next=>{change(next);setCustomPreset(false)}}/>
      <div className="capture-storage"><div><h4>Pasta de gravações e clipes</h4><p className="capture-storage-path">{storage?.directory||"Carregando pasta…"}</p><p>Vale para todos os jogos, organizados em subpastas. Alterar o destino não move os vídeos existentes.</p></div><div className="capture-preview-actions"><button type="button" className="modern-mini-btn" disabled={locked||!storage} onClick={()=>void changeStorage()}>Escolher pasta</button><button type="button" className="modern-mini-btn" disabled={locked||!storage||storage.directory===storage.default_directory} onClick={()=>void changeStorage(true)}>Restaurar padrão</button></div></div>
      <div className="capture-settings-columns"><div><h4>Vídeo</h4><CaptureAudioFields section="video" settings={settings} disabled={locked} onChange={change}/></div>
      <div><h4>Fonte e áudio</h4>
      <div className="capture-preview-field"><span>Modo de captura</span><SelectionMenu label="Modo de captura" disabled={locked} value={settings.method} onChange={method=>change({method})} options={[{value:"portal",label:"Portal · seleção pelo sistema"},{value:"direct",label:"Direta · monitor / auxiliar privilegiado"}]}/></div>
      {settings.method==="portal"?<p>O sistema pede autorização para a janela ou tela que você escolher. Usa PipeWire e o portal do desktop, sem root. Nesta integração, requer Wayland e não captura HDR.</p>:<><p>Grava o monitor inteiro, incluindo desktop e outras janelas. No Wayland, o auxiliar KMS do GPU Screen Recorder pode exigir autorização de administrador.</p><p>Se necessário, o próprio sistema abre o pedido de senha. O PeliGames não recebe nem salva a senha e continua sem root. Se a distribuição já autorizou o auxiliar, o pedido pode não aparecer.</p>
      <div className="capture-preview-field"><span>Monitor</span><SelectionMenu label="Monitor de captura" disabled={locked||!support?.monitors.length} value={monitor} onChange={monitor=>change({monitor})} options={support?.monitors.map(value=>({value,label:value}))||[]}/></div></>}
      {settings.method==="portal"&&support&&!support.portal&&<p role="alert">O portal não foi encontrado nesta sessão. Confira a sessão Wayland e os serviços PipeWire/xdg-desktop-portal.</p>}

      <CaptureAudioFields section="audio" settings={settings} disabled={locked} onChange={change}/>
      </div></div>
      <div className="capture-component-access"><span>GPU Screen Recorder</span><button type="button" className="modern-mini-btn" onClick={openCapturePreferences}>Gerenciar instalação</button></div>
    </section>}
    {tab==="shortcuts"&&<CaptureShortcuts/>}
    {tab==="videos"&&<CaptureVideoLibrary key={gameKey} clips={clips} gameKey={gameKey} gameName={gameName} folder={folder} paused={active||starting} onOpen={open} onRefresh={async()=>{setClips(await invoke<Clip[]>("capture_clips",{gameKey,gameName}));}}/>}

    </div>
    {locked&&tab==="settings"&&<p className="capture-workspace-note">Encerre a captura para alterar os ajustes. Você pode trocar de aba sem interromper a gravação.</p>}
    {notice&&<p className="capture-workspace-note" role="status">{notice}</p>}
    {(error||status.error)&&<p role="alert" className="capture-error">{error||status.error}</p>}
  </div>;
}

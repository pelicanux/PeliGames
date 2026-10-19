import {DebouncedSave} from "../../services/debouncedSave";
import {MakoLaunchSetup,type MakoSandboxStatus} from "./MakoLaunchSetup";
import {FrameOverlaySettings} from "./FrameOverlaySettings";
import {openFrameGenerationPreferences} from "../../services/frameGenerationNavigation";
import {ComponentDownloadProgress} from "../../components/ComponentDownloadProgress";
import {createDownloadRateTracker} from "../../services/downloadProgress";
import {MenuIcon} from "../../components/MenuIcon";
import {ModOptionSwitch} from "../../components/ModOptionRow";
import {useEffect,useRef,useState} from "react";
import {Channel,invoke} from "@tauri-apps/api/core";
import type {GameInfo} from "../../components/GameGrid";
import {isFrameGenerationWrapper,normalizeAdvanced,type LaunchWrapper,type AdvancedSettings} from "../../services/advancedSettings";
import {updatePeliGamesSettings} from "../../services/installedLibrary";
import {openFilePicker} from "../../services/tauriService";
import {notifyError,notifyInfo} from "../../services/notifications";
import {useI18n} from "../../i18n/I18nContext";
import "./frameGeneration.css";
export interface FrameSettings {backend:"mako";runtime_version?:string;enabled:boolean;overlay_enabled:boolean;dll:string;multiplier:number;adaptive:boolean;target_fps:number;flow_scale:number;performance:boolean}
type InstallProgress={phase:"connecting"|"downloading"|"verifying"|"extracting"|"installing"|"complete"|"error";downloaded:number;total:number|null;speed_bytes_per_sec?:number};
const notifiedUpdates=new Set<string>();
type RuntimeVersion={version:string;current:boolean;in_use:boolean;directory:string};
type UpdateInfo={version:string;available:boolean;compatible:boolean;download_url:string;install_directory:string;package_name:string};
type State={update_available:boolean;install_version:string;current_version:string;available_version:string;update_compatible:boolean;installed_versions:RuntimeVersion[];download_url:string;install_directory:string;package_name:string;settings:FrameSettings;versions:Record<string,string>;dll:string|null;installed:boolean;warning:string};
export const isFrameWrapper=isFrameGenerationWrapper;
export function FrameGenerationPanel({game,disabled,running=false,onSaved,onPersist,managementBackend,onBusyChange}:{game:GameInfo;disabled:boolean;running?:boolean;onSaved:(game:GameInfo)=>void;onPersist?:(settings:AdvancedSettings)=>Promise<void>;managementBackend?:"mako";onBusyChange?:(busy:boolean)=>void}){
 const pt=useI18n().language==="pt";
 const [state,setState]=useState<State>();const [settings,setSettings]=useState<FrameSettings>();const [backend,setBackend]=useState<"mako"|"saved">(managementBackend||"saved");const [busy,setBusy]=useState(false);const [error,setError]=useState("");const lock=useRef(false);const request=useRef(0);
 const autosave=useRef<DebouncedSave>(new DebouncedSave());
 const [saveStatus,setSaveStatus]=useState<""|"pending"|"saving"|"saved"|"error">("");
 useEffect(()=>()=>{void autosave.current.flush();},[]);
 const [checking,setChecking]=useState(false);const [backupToDelete,setBackupToDelete]=useState<string>();const checkedBackends=useRef(new Set<string>());
 useEffect(()=>{onBusyChange?.(busy);return()=>onBusyChange?.(false)},[busy,onBusyChange]);
 const [progress,setProgress]=useState<InstallProgress>();
 const [launchSetup,setLaunchSetup]=useState<{game:GameInfo;enabled?:boolean}>();
 const [sandbox,setSandbox]=useState<MakoSandboxStatus>();const [sandboxOpen,setSandboxOpen]=useState(false);const [sandboxBusy,setSandboxBusy]=useState(false);
 const [externalDll,setExternalDll]=useState(false);
 const [overlaySupport,setOverlaySupport]=useState<{available:boolean;reason:string}>();
 useEffect(()=>{let current=true;if(managementBackend)return;setOverlaySupport(undefined);void invoke<{available:boolean;reason:string}>("frame_overlay_support",{directory:game.directory||game.path,executable:game.executable||null,flatpak:Boolean(game.flatpak)}).then(value=>{if(current)setOverlaySupport(value)}).catch(()=>{if(current)setOverlaySupport({available:false,reason:pt?"Não foi possível verificar a arquitetura deste jogo.":"Could not check this game's architecture."})});return()=>{current=false}},[game.directory,game.path,game.executable,game.flatpak,managementBackend,pt]);
 const dllPath=settings?.dll||"";
 useEffect(()=>{
  let current=true;setExternalDll(false);
  if(!dllPath.trim())return;
  const timer=window.setTimeout(()=>{void invoke<boolean|null>("frame_generation_dll_external_filesystem",{path:dllPath}).then(value=>{if(current)setExternalDll(value===true)}).catch(()=>{});},300);
  return()=>{current=false;window.clearTimeout(timer)};
 },[dllPath]);
 const key=game.external_id||game.path;const advanced=normalizeAdvanced(game.advanced);const enabled=(advanced.wrappers ?? []).some(w=>isFrameWrapper(w)&&w.enabled);
 const refresh=async()=>{const id=++request.current;try{const s=await invoke<State>("frame_generation_state",{gameKey:key,backend,activeProfile:(advanced.wrappers ?? []).find(w=>isFrameWrapper(w)&&w.enabled)?.program || null});if(id===request.current){setState(s);if(backend === "saved")setBackend(s.settings.backend);setSettings({...s.settings,enabled:enabled&&s.settings.enabled,overlay_enabled:Boolean(s.settings.overlay_enabled),dll:s.settings.dll||s.dll||""});setError("")}}catch(e){if(id===request.current)setError(String(e))}};
 useEffect(()=>{setState(undefined);setSettings(undefined);setProgress(undefined);void refresh();return()=>{++request.current}},[key,backend]);
 useEffect(()=>{if(managementBackend)return;const reload=()=>{void invoke<State>("frame_generation_state",{gameKey:key,backend,activeProfile:(advanced.wrappers??[]).find(w=>isFrameWrapper(w)&&w.enabled)?.program||null}).then(value=>{setState(value);setSettings(current=>current?{...current,dll:current.dll||value.dll||""}:current)}).catch(failure)};window.addEventListener("frameGenerationRuntimeChanged",reload);return()=>window.removeEventListener("frameGenerationRuntimeChanged",reload)},[backend,key,managementBackend]);
 const checkSandbox=async()=>{if(!settings)return;setSandboxBusy(true);try{const result=await invoke<MakoSandboxStatus>("frame_generation_flatpak_status",{gameKey:key,dll:settings.dll,version:settings.runtime_version||state?.current_version||"4.5.0"});setSandbox(result);setSandboxOpen(true)}catch(e){failure(e)}finally{setSandboxBusy(false)}};
 useEffect(()=>{setSandbox(undefined);setSandboxOpen(false)},[key,dllPath,settings?.runtime_version]);
 const checkUpdates=async(force=false)=>{if(backend==="saved")return;setChecking(true);const selected=backend;try{const result=await invoke<UpdateInfo>("frame_generation_check_updates",{backend:selected,force});setState(old=>old&&old.settings.backend===selected?{...old,available_version:result.version,update_available:result.available,update_compatible:result.compatible,...(result.compatible?{install_version:result.version,download_url:result.download_url,install_directory:result.install_directory,package_name:result.package_name}:{})}:old);if(!force&&result.available&&!notifiedUpdates.has(`${selected}:${result.version}`)){notifiedUpdates.add(`${selected}:${result.version}`);notifyInfo("Frame Generation",pt?`${"MAKO"}: versão estável ${result.version} disponível. Sua versão atual foi preservada.`:`${"MAKO"}: stable ${result.version} available. Your current version is preserved.`,"frame-mako");}if(force)notifyInfo("Frame Generation",result.available?(pt?`Versão estável ${result.version} disponível.`:`Stable ${result.version} available.`):(state?.installed_versions.length?(pt?"Você já tem a versão estável atual.":"You already have the current stable version."):(pt?`Versão estável disponível: ${result.version}.`:`Stable release available: ${result.version}.`)),"frame-mako");}catch(e){if(force)notifyError("Frame Generation",String(e))}finally{setChecking(false)}};
 useEffect(()=>{if(managementBackend&&backend!=="saved"&&!checkedBackends.current.has(backend)){checkedBackends.current.add(backend);void checkUpdates()}},[backend,managementBackend]);
 const removeBackup=async(version:string)=>{if(lock.current)return;lock.current=true;setBusy(true);try{await invoke("frame_generation_remove_backup",{backend,version});setState(old=>old?{...old,installed_versions:old.installed_versions.filter(r=>r.version!==version)}:old);setBackupToDelete(undefined);}catch(e){notifyError("Frame Generation",String(e))}finally{lock.current=false;setBusy(false)}};
 const failure=(e:unknown)=>{setError(String(e));notifyError("Frame Generation",String(e))};
 const install=async()=>{if(lock.current)return;lock.current=true;setBusy(true);try{setProgress({phase:"connecting",downloaded:0,total:null});const onProgress=new Channel<InstallProgress>();const measure=createDownloadRateTracker();onProgress.onmessage=p=>setProgress(measure(p));await invoke("install_frame_generation",{backend,version:state?.install_version,onProgress});await refresh();window.dispatchEvent(new Event("frameGenerationRuntimeChanged"));notifyInfo("Frame Generation",pt?(managementBackend?"Componente estável instalado. Ative-o nos detalhes de cada jogo.":"Componente estável instalado. Configure este jogo para ativá-lo."):(managementBackend?"Stable component installed. Enable it in each game’s details.":"Stable component installed. Configure this game to activate it."))}catch(e){setProgress(p=>({phase:"error",downloaded:p?.downloaded||0,total:p?.total||null}));failure(e)}finally{lock.current=false;setBusy(false)}};
 const saveLive=async(settings:FrameSettings)=>{if(lock.current)return;lock.current=true;setBusy(true);setSaveStatus("saving");try{
 const wrapper=(advanced.wrappers??[]).find(w=>isFrameWrapper(w)&&w.enabled);
 if(!wrapper)throw new Error(pt?"O jogo precisa ter sido iniciado com MAKO para receber ajustes em tempo real.":"The game must have launched with MAKO to accept live settings.");
 await invoke("update_frame_generation_live",{activeProfile:wrapper.program,settings});
 setError("");setSaveStatus("saved");
 }catch(e){setSaveStatus("error");setError(String(e))}finally{lock.current=false;setBusy(false)}};
 const save=async(settings:FrameSettings)=>{if(managementBackend||lock.current)return;lock.current=true;setBusy(true);setSaveStatus("saving");try{
  const wrapper=await invoke<LaunchWrapper>("prepare_frame_generation",{gameKey:key,gameName:game.name,directory:game.directory||game.path,settings,advanced,flatpak:Boolean(game.flatpak),executable:game.executable||null});
  const launchChanged=settings.enabled?!(advanced.wrappers??[]).some(w=>isFrameWrapper(w)&&w.enabled&&w.program===wrapper.program):enabled;
  const next={...advanced,wrappers:[...(advanced.wrappers ?? []).filter(w=>!isFrameWrapper(w)),...(settings.enabled?[wrapper]:[])]};
  const updated=!launchChanged?game:onPersist?await onPersist(next).then(()=>({...game,advanced:next})):game.external_id?await invoke<GameInfo>("configure_external_game",{settings:{external_id:game.external_id,advanced:next}}):await updatePeliGamesSettings({path:game.path,name:game.name,executable:game.executable||game.path,proton:game.proton||"",prefix:game.prefix,advanced:next});
  const saved={...game,...updated,library_view:game.library_view};onSaved(saved);if(launchChanged&&(saved.launcher==="Steam"?saved.steam_pending:settings.enabled&&saved.launcher!=="PeliGames"))setLaunchSetup({game:saved,enabled:settings.enabled});window.dispatchEvent(new Event("peligamesLibraryChanged"));setSaveStatus("saved");setError("");
 }catch(e){setSaveStatus("error");setError(String(e))}finally{lock.current=false;setBusy(false)}};
 const patch=(v:Partial<FrameSettings>)=>{
   if(!settings)return;
   const next={...settings,...v};
   setSettings(next);setError("");setSaveStatus("pending");
   autosave.current.schedule(()=>running?saveLive(next):save(next));
 };

 const selectedVersion=settings?.runtime_version||state?.current_version;const selectedRuntime=state?.installed_versions.find(r=>r.version===selectedVersion);const componentInstalled=game.flatpak?Boolean(sandbox?.ready):Boolean(selectedRuntime);const updateAvailable=Boolean(state?.update_available);
 const blocked=busy||disabled;const startupBlocked=blocked||running;const liveUnavailable=running&&!enabled;const supported=Boolean(onPersist)||game.launcher==="PeliGames"||Boolean(game.external_id);
 const installationProgress = progress&&<ComponentDownloadProgress progress={progress} onDismiss={()=>setProgress(undefined)} label={({connecting:pt?"Conectando à origem…":"Connecting…",downloading:pt?"Baixando componente…":"Downloading…",verifying:pt?"Verificando pacote…":"Verifying package…",extracting:pt?"Extraindo arquivos…":"Extracting files…",installing:pt?"Concluindo instalação…":"Finishing installation…",complete:pt?"Instalação concluída":"Installation complete",error:pt?"Instalação não concluída":"Installation failed"})[progress.phase]}/>;
 if(managementBackend)return <section className="frame-generation-panel fg-runtime-management mod-workspace">
  <header className="mod-intro"><h3>{pt?"Preparar geração de quadros":"Prepare frame generation"}</h3><p>{pt?"Gere quadros adicionais com o Lossless Scaling. Prepare o componente aqui e escolha em quais jogos usar.":"Generate extra frames with Lossless Scaling. Prepare the component here, then choose which games use it."}</p></header>
  <section className="mod-install-surface" aria-label={pt?"Instalação do componente":"Component installation"}>
    <div className="mod-step-heading"><span className="mod-step-number" data-complete={componentInstalled}>{componentInstalled?<MenuIcon name="check"/>:"1"}</span><div><h4>{pt?"Prepare o componente":"Prepare the component"}</h4><p>{pt?"Uma instalação compartilhada entre os jogos.":"One installation shared across games."}</p></div><span className="mod-status-badge" data-ready={componentInstalled} role="status">{!state?(pt?"Verificando…":"Checking…"):componentInstalled?`${pt?"Instalado":"Installed"} · ${selectedVersion}`:(pt?"Não instalado":"Not installed")}</span></div>
    <div className="mod-step-actions">
      {state&&(!componentInstalled||(updateAvailable&&state.update_compatible))&&<button type="button" className="mod-action-primary" disabled={blocked||checking} onClick={()=>void install()}><MenuIcon name="download"/>{busy?(pt?"Instalando…":"Installing…"):updateAvailable&&state.update_compatible?(pt?`Atualizar para ${state.available_version}`:`Update to ${state.available_version}`):`${pt?"Instalar":"Install"} ${"MAKO"}`}</button>}
      <button type="button" className="mod-action-secondary" disabled={blocked||checking||!state} onClick={()=>void checkUpdates(true)}><MenuIcon name="repair"/>{checking?(pt?"Verificando…":"Checking…"):(pt?"Buscar atualização":"Check for updates")}</button>
    </div>
    {installationProgress}
    {error&&<p className="mod-feedback-error" role="alert">{error}</p>}
    {state&&<div className="mod-destination"><MenuIcon name="folder"/><div><span>{pt?"Destino do componente":"Component location"}</span><button type="button" disabled={!state.installed_versions.some(r=>r.directory===state.install_directory)} title={state.install_directory} onClick={()=>void invoke("open_folder",{path:state.install_directory}).catch(failure)}>{state.install_directory}</button></div></div>}
  </section>
  <section className="mod-next-step"><div className="mod-step-heading"><span className="mod-step-number">2</span><div><h4>{pt?"Ative nos jogos que você escolher":"Enable it in the games you choose"}</h4><p>{pt?"Biblioteca → selecione um jogo → Geração de quadros.":"Library → select a game → Frame generation."}</p></div></div><p className="mod-step-note">{pt?"Escolha a DLL do Lossless Scaling, ajuste o multiplicador e ative para esse jogo. Instalar o componente não ativa a geração automaticamente.":"Choose the Lossless Scaling DLL, set the multiplier and enable it for that game. Installing the component does not enable frame generation automatically."}</p>
  <div className="mod-requirement"><MenuIcon name="info"/><p>{pt?"Tenha o Lossless Scaling na Steam, na versão pública (Lossless.dll).":"Requires the public Steam version of Lossless Scaling (Lossless.dll)."} {pt?"A DLL não acompanha este download.":"The DLL is not included in this download."}</p></div></section>
  {state&&<>
    {updateAvailable&&!state.update_compatible&&<p className="fg-partition-note">{pt?`A versão ${state.available_version} requer adaptação do PeliGames. A versão atual foi preservada.`:`Version ${state.available_version} requires a PeliGames integration update. The current version was preserved.`}</p>}
    {state.installed_versions.length>0&&<details className="fg-version-history"><summary>{pt?"Versões e backups":"Versions and backups"} <small>({state.installed_versions.length})</small></summary><p>{pt?"Atualizações preservam as versões anteriores. Para usar um backup, escolha a versão nos detalhes do jogo.":"Updates preserve older versions. To use a backup, choose its version in the game's details."}</p>{state.installed_versions.map(r=><div className="fg-version-row" key={r.version}><span>{r.version}<small>{r.current?(pt?"Atual":"Current"):(pt?"Backup":"Backup")}{r.in_use?pt?" · vinculado a perfis":" · linked to profiles":""}</small></span>{!r.current&&<button type="button" className="modern-mini-btn fg-delete-backup" disabled={blocked||r.in_use} aria-label={pt?`Excluir backup ${r.version}`:`Delete backup ${r.version}`} title={r.in_use?(pt?"Vinculado a perfis; remoção bloqueada.":"Linked to profiles; removal blocked."):(pt?"Excluir backup":"Delete backup")} onClick={()=>setBackupToDelete(r.version)}><MenuIcon name="trash"/></button>}</div>)}{backupToDelete&&<div className="fg-delete-confirm"><span>{pt?`Excluir permanentemente o backup ${backupToDelete}?`:`Permanently delete backup ${backupToDelete}?`}</span><button type="button" className="modern-mini-btn danger" disabled={blocked} onClick={()=>void removeBackup(backupToDelete)}>{pt?"Excluir backup":"Delete backup"}</button><button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>setBackupToDelete(undefined)}>{pt?"Cancelar":"Cancel"}</button></div>}</details>}
  </>}
 </section>;
 return <section className="frame-generation-panel fg-game-workflow">
  {error&&<p role="alert">{error}</p>}
  {launchSetup&&<MakoLaunchSetup game={launchSetup.game} enabled={launchSetup.enabled} onSaved={onSaved} onClose={()=>setLaunchSetup(undefined)}/>}
  {sandboxOpen&&sandbox&&<MakoLaunchSetup game={game} sandbox={sandbox} onCheckSandbox={checkSandbox} onSaved={onSaved} onClose={()=>setSandboxOpen(false)}/>}
  {game.flatpak&&<div className="fg-sandbox-setup"><span>{sandbox?.ready?(pt?"Flatpak preparado":"Flatpak prepared"):(pt?"O Flatpak precisa de extensão e acesso ao perfil e à DLL.":"Flatpak requires an extension and access to the profile and DLL.")}</span><button type="button" className="modern-mini-btn" disabled={sandboxBusy||busy||!settings} onClick={()=>void checkSandbox()}>{sandboxBusy?(pt?"Verificando…":"Checking…"):(pt?"Preparar / verificar Flatpak":"Prepare / verify Flatpak")}</button></div>}
  <div className="modern-dlss-panels fg-game-grid">
   <section className="modern-dlss-card fg-game-setup" aria-label={pt?"Configurar geração de quadros":"Configure frame generation"}>
    <h3>{pt?"Preparar para este jogo":"Prepare for this game"}</h3>
    <p className="modern-dlss-description">{pt?"Confira a DLL do MAKO e ajuste o multiplicador. As alterações são salvas automaticamente para a próxima execução.":"Check the MAKO DLL and set the multiplier. Changes save automatically for the next launch."}</p>
  <div className="fg-toolbar">
   {state&&settings&&<div className="fg-master-toggle"><span>{pt?"Ativar neste jogo":"Enable for this game"}</span><ModOptionSwitch label={pt?"Ativar neste jogo":"Enable for this game"} enabled={settings.enabled} disabled={startupBlocked||!componentInstalled||!supported} onToggle={()=>patch({enabled:!settings.enabled})}/></div>}
   <span className="fg-generator-name">MAKO</span>
  </div>

    {!game.flatpak&&state&&!componentInstalled&&<div className="fg-component-row"><button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>{if(backend==="saved")return;notifyInfo("Frame Generation",pt?`${"MAKO"} não está instalado. Instale o componente em Configurações → Mods.`:`${"MAKO"} is not installed. Install it in Settings → Mods.`,"frame-mako");openFrameGenerationPreferences(backend);}}>{pt?"Instalar":"Install"}</button></div>}
    {!state||!settings?<p role="status">{pt?"Verificando componente e perfil…":"Checking component and profile…"}</p>:<div className="fg-source">
  <p>{pt?"Requer a Lossless.dll da versão pública padrão do Lossless Scaling na Steam.":"Requires Lossless.dll from the default public Steam version of Lossless Scaling."}</p>
  <div className="fg-dll"><label>DLL<input value={settings.dll} disabled={startupBlocked} onChange={e=>patch({dll:e.target.value})} placeholder={state.dll||"Lossless.dll"}/></label><button className="modern-mini-btn" disabled={startupBlocked} onClick={()=>void openFilePicker("Lossless Scaling",["dll"]).then(p=>{if(p)patch({dll:p})}).catch(failure)}>{pt?"Localizar":"Browse"}</button></div>
   <div className="modern-dlss-setting fg-control-field fg-setting"><span>{pt?(settings.adaptive&&backend==="mako"?"Multiplicador máximo":"Multiplicador"):"Multiplier"}</span><div className="modern-dlss-segments fg-multipliers" role="group" aria-label={pt?"Multiplicador":"Multiplier"}>{[2,3,4,5].map(n=><button type="button" key={n} aria-pressed={settings.multiplier===n} disabled={blocked||liveUnavailable} onClick={()=>patch({multiplier:n})}>{n}×</button>)}</div></div>
  <dl className="fg-download-info"><div><dt>{pt?"Destino":"Destination"}</dt><dd><button type="button" title={state.installed_versions.some(r=>r.directory===state.install_directory)?state.install_directory:(pt?"Pasta disponível após a instalação: ":"Folder available after installation: ")+state.install_directory} disabled={!state.installed_versions.some(r=>r.directory===state.install_directory)} onClick={()=>void invoke("open_folder",{path:state.install_directory}).catch(failure)}>{state.install_directory}</button></dd></div></dl>
  {state.installed_versions.length>0&&<details className="fg-version-history"><summary>{pt?"Versões e backups":"Versions and backups"} <small>({state.installed_versions.length})</small></summary><p>{pt?"Atualizar preserva as versões anteriores. Troque abaixo para aplicar somente a este jogo.":"Updates preserve older versions. Select below to apply only to this game."}</p>{state.installed_versions.map(r=><div className="fg-version-row" key={r.version}><span>{r.version}<small>{r.current?(pt?"Atual":"Current"):(pt?"Backup":"Backup")}{r.in_use?pt?" · vinculado a perfis":" · linked to profiles":""}</small></span><div><button type="button" className="modern-mini-btn" disabled={startupBlocked||r.version===selectedVersion} onClick={()=>patch({runtime_version:r.version})}>{r.version===selectedVersion?(pt?"Selecionada":"Selected"):(pt?"Usar neste jogo":"Use for this game")}</button></div></div>)}</details>}
  {externalDll&&<p className="fg-partition-note" role="note">{pt?"DLL em outro sistema de arquivos. Se o gerador não funcionar, o runtime da Steam pode estar impedindo o acesso. Tente uma cópia na sua pasta pessoal e selecione-a aqui.":"DLL on another filesystem. If frame generation does not work, Steam’s runtime may be blocking access. Try a copy in your home folder and select it here."}</p>}

    {running&&<p role="status">{liveUnavailable?(pt?"Este jogo não tem MAKO ativado no perfil. Encerre o jogo para habilitá-lo.":"MAKO is not enabled in this profile. Close the game to enable it."):(pt?"Jogo em execução: multiplicador, adaptativo, FPS alvo, escala de movimento e modelo leve podem ser ajustados aqui. DLL, versão e ativação exigem encerrar o jogo.":"Game running: adjust multiplier, adaptive mode, target FPS, flow scale and lighter model here. DLL, version and activation require closing the game.")}</p>}
    <div className="modern-dlss-actions fg-game-actions">
  <span role="status" className="fg-autosave-status">{saveStatus==="saving"?(pt?"Salvando…":"Saving…"):saveStatus==="pending"?(pt?"Alterações serão salvas automaticamente":"Changes will save automatically"):saveStatus==="error"?(pt?"Não foi possível salvar. Ajuste a opção ou tente novamente.":"Could not save. Adjust the option or retry."):saveStatus==="saved"?(pt?"Salvo automaticamente":"Saved automatically"):(pt?"As alterações são salvas automaticamente":"Changes save automatically")}</span>
  {saveStatus==="error"&&<button type="button" className="modern-mini-btn" disabled={blocked||liveUnavailable||!supported} onClick={()=>{setSaveStatus("pending");autosave.current.schedule(()=>running?saveLive(settings):save(settings));}}>{pt?"Tentar novamente":"Retry"}</button>}
  {game.steam_pending&&<p>{pt?"Há alterações pendentes na Steam. Use as opções de aplicar, configurar manualmente ou reverter junto ao botão Jogar.":"Steam has pending changes. Apply, configure manually or revert beside the Play button."}</p>}

    </div>
    </div>}
   </section>
   <section className="modern-dlss-card fg-game-options" aria-label={pt?"Opções da geração de quadros":"Frame generation options"}>
    <h3>{pt?"Opções":"Options"}</h3><p className="modern-dlss-description">{pt?"Refine a geração e confira a compatibilidade antes de ativar.":"Fine-tune generation and check compatibility before enabling."}</p>
    {state&&settings&&<><div className="fg-controls">
   {backend==="mako"&&<div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{pt?"Modo adaptativo":"Adaptive mode"}</span><small>{pt?"Ajusta os quadros extras conforme a cena.":"Adjusts extra frames to the scene."}</small></span><ModOptionSwitch label={pt?"Modo adaptativo":"Adaptive mode"} enabled={settings.adaptive} disabled={blocked||liveUnavailable} onToggle={()=>patch({adaptive:!settings.adaptive})}/></div>}

   {backend==="mako"&&settings.adaptive&&<label className="modern-dlss-setting fg-control-field fg-setting">{pt?"FPS de saída desejado":"Target output FPS"}<input type="number" min={10} max={1000} value={settings.target_fps} disabled={blocked||liveUnavailable} onChange={e=>patch({target_fps:Number(e.target.value)})}/></label>}
   <label className="modern-dlss-setting fg-control-field fg-setting fg-flow"><span className="fg-setting-copy"><span>{pt?"Escala de movimento":"Flow scale"}</span><small>{pt?"Resolução usada para estimar o movimento.":"Resolution used to estimate motion."}</small></span><span className="fg-flow-value"><output>{Math.round(settings.flow_scale*100)}%</output><input type="range" aria-label={pt?"Escala de movimento":"Flow scale"} min={25} max={100} value={Math.round(settings.flow_scale*100)} disabled={blocked||liveUnavailable} onChange={e=>patch({flow_scale:Number(e.target.value)/100})}/></span></label>
   <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{pt?"Modelo leve":"Lighter model"}</span><small>{pt?"Prioriza desempenho na geração de quadros.":"Prioritizes frame generation performance."}</small></span><ModOptionSwitch label={pt?"Modelo leve":"Lighter model"} enabled={settings.performance} disabled={blocked||liveUnavailable} onToggle={()=>patch({performance:!settings.performance})}/></div>

    </div>
  <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{pt?"Overlay PeliGames":"PeliGames overlay"} <small>{pt?"Experimental":"Experimental"}</small></span><small>{pt?"Carrega com o jogo e começa oculto. Use o atalho para mostrar os controles e arrastar o painel.":"Loads with the game and starts hidden. Use the shortcut to show the controls and drag the panel."}</small></span><ModOptionSwitch label={pt?"Overlay PeliGames neste jogo":"PeliGames overlay for this game"} enabled={settings.overlay_enabled} disabled={startupBlocked||(!settings.overlay_enabled&&(!overlaySupport?.available||Boolean(game.flatpak)))} onToggle={()=>patch({overlay_enabled:!settings.overlay_enabled})}/></div>
  <p className="fg-overlay-support-note">{overlaySupport?.reason||(pt?"Vulkan de 64 bits; compatibilidade gráfica ainda em validação.":"64-bit Vulkan; graphics compatibility is still being validated.")}</p>
  <FrameOverlaySettings compact disabled={startupBlocked}/>

  <div className="fg-compatibility"><strong>{pt?"Compatibilidade":"Compatibility"}</strong><p>{state.warning}</p><p>{pt?"O MangoHUD pode contar apenas os quadros reais. Mais FPS de saída não reduz a latência na mesma proporção. Desative qualquer outro gerador antes de testar.":"MangoHUD may count only real frames. Higher output FPS does not proportionally reduce input latency. Disable other frame generators before testing."}</p>
   {backend==="mako"&&advanced.options.hdr&&<p role="alert">{pt?"Desative HDR para usar esta integração estável do MAKO, configurada para SDR. O HDR do MAKO ainda é experimental.":"Disable HDR for this stable MAKO integration, configured for SDR. MAKO HDR is still experimental."}</p>}
   {game.flatpak&&<p role="alert">{pt?"O Flatpak usa sua própria extensão MAKO. Use Preparar / verificar Flatpak antes de ativar.":"Flatpak uses its own MAKO extension. Prepare / verify Flatpak before enabling."}</p>}
   {!supported&&<p>{pt?"Registre esta instalação na biblioteca principal para configurar sua execução.":"Register this installation in the main library to configure its launch."}</p>}
   {(advanced.options.mlfg||advanced.options.obs_capture||advanced.options.vkd3d_latency||advanced.options.vulkan_latency)&&<p role="alert">{pt?"Desative geração ML, baixa latência DX12, camada Vulkan de baixa latência e captura Vulkan OBS nas opções avançadas antes de ativar este recurso.":"Disable ML frame generation in advanced options first."}</p>}

  </div>

    </>}
   </section>
  </div>
 </section>
}

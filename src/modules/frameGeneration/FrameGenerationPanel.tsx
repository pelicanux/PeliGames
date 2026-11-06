import {ConfirmModal} from "../../components/ConfirmModal";
import {SelectionMenu} from "../../components/SelectionMenu";
import {MakoAdditionalControls} from "./MakoAdditionalControls";
import {useLinuxGpus} from "../../hooks/useLinuxGpus";
import {useGameSaveStatus, GameSaveStatusIndicator} from "../../components/GameSaveStatus";
import {DebouncedSave} from "../../services/debouncedSave";
import {MakoLaunchSetup,type MakoSandboxStatus} from "./MakoLaunchSetup";
import {openFrameGenerationPreferences} from "../../services/frameGenerationNavigation";
import {ComponentDownloadProgress} from "../../components/ComponentDownloadProgress";
import {createDownloadRateTracker} from "../../services/downloadProgress";
import {MenuIcon} from "../../components/MenuIcon";
import {ModOptionSwitch} from "../../components/ModOptionRow";
import {useEffect,useId,useRef,useState} from "react";
import {Channel,invoke} from "@tauri-apps/api/core";
import type {GameInfo} from "../../components/GameGrid";
import {isFrameGenerationWrapper,normalizeAdvanced,type LaunchWrapper,type AdvancedSettings} from "../../services/advancedSettings";
import {selectFrameGenerator} from "../../services/frameGenerationSelection";
import {updatePeliGamesSettings} from "../../services/installedLibrary";
import {openFilePicker} from "../../services/tauriService";
import {notifyError,notifyInfo} from "../../services/notifications";
import {useI18n} from "../../i18n/I18nContext";
import "./frameGeneration.css";
export interface FrameSettings {backend:"mako";runtime_version?:string;enabled:boolean;overlay_enabled:boolean;dll:string;multiplier:number;adaptive:boolean;target_fps:number;flow_scale:number;performance:boolean;gpu?:string;base_fps_cap?:number;refresh_threshold?:number;ultra_performance?:boolean;allow_fp16?:boolean;live_enabled?:boolean;scaling_enabled?:boolean;scaling_method?:string;scaling_factor?:number;scaling_sharpness?:number}
type InstallProgress={phase:"connecting"|"downloading"|"verifying"|"extracting"|"installing"|"complete"|"error";downloaded:number;total:number|null;speed_bytes_per_sec?:number};
const notifiedUpdates=new Set<string>();
type RuntimeVersion={version:string;current:boolean;in_use:boolean;directory:string};
type UpdateInfo={version:string;available:boolean;compatible:boolean;download_url:string;install_directory:string;package_name:string};
type State={update_available:boolean;install_version:string;current_version:string;available_version:string;update_compatible:boolean;installed_versions:RuntimeVersion[];download_url:string;install_directory:string;package_name:string;settings:FrameSettings;versions:Record<string,string>;dll:string|null;installed:boolean;warning:string};
export const isFrameWrapper=isFrameGenerationWrapper;
export function FrameGenerationPanel({game,disabled,running=false,onSaved,onPersist,managementBackend,onBusyChange}:{game:GameInfo;disabled:boolean;running?:boolean;onSaved:(game:GameInfo)=>void;onPersist?:(settings:AdvancedSettings)=>Promise<void>;managementBackend?:"mako"|"afmf";onBusyChange?:(busy:boolean)=>void}){
  const {t: tx} = useI18n();

 const versionsTitleId=useId();
 const {gpus,error:gpuError}=useLinuxGpus();
 const afmf=managementBackend==="afmf";const generator=afmf?"AFMF":"MAKO";const noticeAction=afmf?"frame-afmf" as const:"frame-mako" as const;
 const [state,setState]=useState<State>();const [settings,setSettings]=useState<FrameSettings>();const [backend,setBackend]=useState<"mako"|"afmf"|"saved">(managementBackend||"saved");const [busy,setBusy]=useState(false);const [error,setError]=useState("");const lock=useRef(false);const request=useRef(0);
 const autosave=useRef<DebouncedSave>(new DebouncedSave());
 const [saveStatus,setSaveStatus]=useState<""|"pending"|"saving"|"saved"|"error">("");
 const [savePanel,setSavePanel]=useState<"fg-prepare"|"fg-options">("fg-prepare");
 const headerStatus=useGameSaveStatus(saveStatus,savePanel);
 useEffect(()=>()=>{void autosave.current.flush();},[]);
 const [uninstallConfirm,setUninstallConfirm]=useState(false);
 const uninstall=async()=>{if(lock.current)return;lock.current=true;setBusy(true);setError("");try{await invoke("frame_generation_uninstall",{backend});setUninstallConfirm(false);await refresh();window.dispatchEvent(new Event("frameGenerationRuntimeChanged"));}catch(e){setError(String(e));setUninstallConfirm(false);}finally{lock.current=false;setBusy(false);}};
 const [checking,setChecking]=useState(false);const [backupToDelete,setBackupToDelete]=useState<string>();const checkedBackends=useRef(new Set<string>());
 useEffect(()=>{onBusyChange?.(busy);return()=>onBusyChange?.(false)},[busy,onBusyChange]);
 const [progress,setProgress]=useState<InstallProgress>();
 const [launchSetup,setLaunchSetup]=useState<{game:GameInfo;enabled?:boolean}>();
 const [sandbox,setSandbox]=useState<MakoSandboxStatus>();const [sandboxOpen,setSandboxOpen]=useState(false);const [sandboxBusy,setSandboxBusy]=useState(false);
 const [externalDll,setExternalDll]=useState(false);
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
 const checkUpdates=async(force=false)=>{if(backend==="saved")return;setChecking(true);const selected=backend;try{const result=await invoke<UpdateInfo>("frame_generation_check_updates",{backend:selected,force});setState(old=>old&&old.settings.backend===selected?{...old,available_version:result.version,update_available:result.available,update_compatible:result.compatible,...(result.compatible?{install_version:result.version,download_url:result.download_url,install_directory:result.install_directory,package_name:result.package_name}:{})}:old);if(!force&&result.available&&!notifiedUpdates.has(`${selected}:${result.version}`)){notifiedUpdates.add(`${selected}:${result.version}`);notifyInfo("Frame Generation",tx("ui-frame-generation-panel", "stable_available_your_current_version_is_preserved", {"generator": generator, "result_version": result.version}),noticeAction);}if(force)notifyInfo("Frame Generation",result.available?(tx("ui-frame-generation-panel", "stable_available", {"result_version": result.version})):(state?.installed_versions.length?(tx("ui-frame-generation-panel", "you_already_have_the_current_stable_version")):(tx("ui-frame-generation-panel", "stable_release_available", {"result_version": result.version}))),noticeAction);}catch(e){if(force)notifyError("Frame Generation",String(e))}finally{setChecking(false)}};
 useEffect(()=>{if(managementBackend&&backend!=="saved"&&!checkedBackends.current.has(backend)){checkedBackends.current.add(backend);void checkUpdates()}},[backend,managementBackend]);
 const removeBackup=async(version:string)=>{if(lock.current)return;lock.current=true;setBusy(true);try{await invoke("frame_generation_remove_backup",{backend,version});setState(old=>old?{...old,installed_versions:old.installed_versions.filter(r=>r.version!==version)}:old);setBackupToDelete(undefined);}catch(e){notifyError("Frame Generation",String(e))}finally{lock.current=false;setBusy(false)}};
 const failure=(e:unknown)=>{setError(String(e));notifyError("Frame Generation",String(e))};
 const install=async()=>{if(lock.current)return;lock.current=true;setBusy(true);try{setProgress({phase:"connecting",downloaded:0,total:null});const onProgress=new Channel<InstallProgress>();const measure=createDownloadRateTracker();onProgress.onmessage=p=>setProgress(measure(p));await invoke("install_frame_generation",{backend,version:state?.install_version,onProgress});await refresh();window.dispatchEvent(new Event("frameGenerationRuntimeChanged"));notifyInfo("Frame Generation",(managementBackend ? tx("ui-frame-generation-panel", "stable_component_installed_enable_it_in_each_game_s_details") : tx("ui-frame-generation-panel", "stable_component_installed_configure_this_game_to_activate_it")))}catch(e){setProgress(p=>({phase:"error",downloaded:p?.downloaded||0,total:p?.total||null}));failure(e)}finally{lock.current=false;setBusy(false)}};
 const saveLive=async(settings:FrameSettings)=>{if(lock.current)return;lock.current=true;setBusy(true);setSaveStatus("saving");try{
 const wrapper=(advanced.wrappers??[]).find(w=>isFrameWrapper(w)&&w.enabled);
 if(!wrapper)throw new Error(tx("ui-frame-generation-panel", "the_game_must_have_launched_with_mako_to_accept_live_settings"));
 await invoke("update_frame_generation_live",{activeProfile:wrapper.program,settings});
 window.dispatchEvent(new Event("frameGenerationProfileChanged"));
 setError("");setSaveStatus("saved");
 }catch(e){setSaveStatus("error");setError(String(e))}finally{lock.current=false;setBusy(false)}};
 const save=async(settings:FrameSettings)=>{if(managementBackend||lock.current)return;lock.current=true;setBusy(true);setSaveStatus("saving");try{
  const selectedAdvanced=settings.enabled?selectFrameGenerator(advanced,"mako"):advanced;
  const wrapper=await invoke<LaunchWrapper>("prepare_frame_generation",{gameKey:key,gameName:game.name,directory:game.directory||game.path,settings,advanced:selectedAdvanced,flatpak:Boolean(game.flatpak),executable:game.executable||null});
  const launchChanged=selectedAdvanced!==advanced||(settings.enabled?!(advanced.wrappers??[]).some(w=>isFrameWrapper(w)&&w.enabled&&w.program===wrapper.program):enabled)||(advanced.overlay_enabled==null&&settings.overlay_enabled);
  const next={...selectedAdvanced,overlay_enabled:advanced.overlay_enabled??(settings.overlay_enabled?true:null),wrappers:[...(advanced.wrappers ?? []).filter(w=>!isFrameWrapper(w)),...(settings.enabled?[wrapper]:[])]};
  const updated=!launchChanged?game:onPersist?await onPersist(next).then(()=>({...game,advanced:next})):game.external_id?await invoke<GameInfo>("configure_external_game",{settings:{external_id:game.external_id,advanced:next}}):await updatePeliGamesSettings({path:game.path,name:game.name,executable:game.executable||game.path,proton:game.proton||"",prefix:game.prefix,advanced:next});
  const saved={...game,...updated,library_view:game.library_view};onSaved(saved);if(launchChanged&&(saved.launcher==="Steam"?saved.steam_pending:settings.enabled&&saved.launcher!=="PeliGames"))setLaunchSetup({game:saved,enabled:settings.enabled});window.dispatchEvent(new Event("peligamesLibraryChanged"));setSaveStatus("saved");setError("");
 }catch(e){setSaveStatus("error");setError(String(e))}finally{lock.current=false;setBusy(false)}};
 const patch=(v:Partial<FrameSettings>,panel:"fg-prepare"|"fg-options"="fg-prepare")=>{
   if(!settings)return;
   const next={...settings,...v};
   if ((next.base_fps_cap??0)<0||(next.base_fps_cap??0)>1000||(next.refresh_threshold??0)<0||(next.refresh_threshold??0)>1000) {failure("Informe um limite entre 0 e 1000.");return;}

   setSavePanel(panel);setSettings(next);setError("");setSaveStatus("pending");
   autosave.current.schedule(()=>running?saveLive(next):save(next));
 };

 const selectedVersion=settings?.runtime_version||state?.current_version;const selectedRuntime=state?.installed_versions.find(r=>r.version===selectedVersion);const componentInstalled=game.flatpak?Boolean(sandbox?.ready):Boolean(selectedRuntime);const updateAvailable=Boolean(state?.update_available);
 const blocked=busy||disabled;const startupBlocked=blocked||running;const liveUnavailable=running&&!enabled;const supported=Boolean(onPersist)||game.launcher==="PeliGames"||Boolean(game.external_id);
 const installationProgress = progress&&<ComponentDownloadProgress progress={progress} onDismiss={()=>setProgress(undefined)} label={({connecting:tx("ui-frame-generation-panel", "connecting"),downloading:tx("ui-frame-generation-panel", "downloading"),verifying:tx("ui-frame-generation-panel", "verifying_package"),extracting:tx("ui-frame-generation-panel", "extracting_files"),installing:tx("ui-frame-generation-panel", "finishing_installation"),complete:tx("ui-frame-generation-panel", "installation_complete"),error:tx("ui-frame-generation-panel", "installation_failed")})[progress.phase]}/>;
 if(managementBackend)return <section className="frame-generation-panel fg-runtime-management mod-workspace">
  <header className="mod-intro"><h3>{afmf?(tx("ui-frame-generation-panel", "prepare_afmf")):(tx("ui-frame-generation-panel", "prepare_frame_generation"))}</h3><p>{afmf?(tx("ui-frame-generation-panel", "afmf_generates_one_in_between_frame_per_real_frame_2_with_a_vulkan_layer_for_linux_insta")):(tx("ui-frame-generation-panel", "generate_extra_frames_with_lossless_scaling_prepare_the_component_here_then_choose_which"))}</p></header>
  <section className="mod-install-surface" aria-label={tx("ui-frame-generation-panel", "component_installation")}>
    <div className="mod-step-heading"><span className="mod-step-number" data-complete={componentInstalled}>{componentInstalled?<MenuIcon name="check"/>:"1"}</span><div><h4>{tx("ui-frame-generation-panel", "prepare_the_component")}</h4><p>{tx("ui-frame-generation-panel", "one_installation_shared_across_games")}</p></div><span className="mod-status-badge" data-ready={componentInstalled} role="status">{!state?(tx("ui-frame-generation-panel", "checking")):componentInstalled?`${tx("ui-frame-generation-panel", "installed")} · ${selectedVersion}`:(tx("ui-frame-generation-panel", "not_installed"))}</span></div>
    <div className="mod-step-actions">
      {state&&(!componentInstalled||(updateAvailable&&state.update_compatible))&&<button type="button" className="mod-action-primary" disabled={blocked||checking} onClick={()=>void install()}><MenuIcon name="download"/>{busy?(tx("ui-frame-generation-panel", "installing")):updateAvailable&&state.update_compatible?(tx("ui-frame-generation-panel", "update_to", {"state_available_version": state.available_version})):`${tx("ui-frame-generation-panel", "install")} ${generator}`}</button>}
      <button type="button" className="mod-action-secondary" disabled={blocked||checking||!state} onClick={()=>void checkUpdates(true)}><MenuIcon name="repair"/>{checking?(tx("ui-frame-generation-panel", "checking")):(tx("ui-frame-generation-panel", "check_for_updates"))}</button>
    </div>
    {installationProgress}
    {error&&<p className="mod-feedback-error" role="alert">{error}</p>}
    {state&&<div className="mod-destination"><MenuIcon name="folder"/><div><span>{tx("ui-frame-generation-panel", "component_location")}</span><button type="button" disabled={!state.installed_versions.some(r=>r.directory===state.install_directory)} title={state.install_directory} onClick={()=>void invoke("open_folder",{path:state.install_directory}).catch(failure)}>{state.install_directory}</button></div></div>}
  </section>
  <section className="mod-next-step"><div className="mod-step-heading"><span className="mod-step-number">2</span><div><h4>{tx("ui-frame-generation-panel", "enable_it_in_the_games_you_choose")}</h4><p>{tx("ui-frame-generation-panel", "library_select_a_game_frame_generation")}</p></div></div><p className="mod-step-note">{afmf?(tx("ui-frame-generation-panel", "in_the_game_s_frame_generation_tab_choose_afmf_enable_it_and_adjust_the_options_installi")):(tx("ui-frame-generation-panel", "choose_the_lossless_scaling_dll_set_the_multiplier_and_enable_it_for_that_game_installin"))}</p>
  <div className="mod-requirement"><MenuIcon name="info"/><p>{afmf?(tx("ui-frame-generation-panel", "tested_on_amd_rx_7000_and_rx_9000_cards_with_radv_needs_vulkan_1_1_and_a_64_bit_process_")):<>{tx("ui-frame-generation-panel", "requires_the_public_steam_version_of_lossless_scaling_lossless_dll")} {tx("ui-frame-generation-panel", "the_dll_is_not_included_in_this_download")}</>}</p></div></section>
  {state&&state.installed_versions.length>0&&<div className="mod-backend-tools"><div className="mod-row-copy"><h4>{tx("ui-frame-generation-panel", "uninstall", {"generator": generator})}</h4><p>{tx("ui-frame-generation-panel", "removes_the_component_and_its_local_versions_game_settings_and_the_lossless_scaling_dll_")}</p>{state.installed_versions.some(r=>r.in_use)&&<p role="status">{tx("ui-frame-generation-panel", "disable_the_component_in_games_and_apply_pending_changes_before_uninstalling")}</p>}</div><div className="mod-tool-actions"><button type="button" className="mod-action-quiet danger" disabled={blocked||checking} onClick={()=>setUninstallConfirm(true)}><MenuIcon name="trash"/>{tx("ui-frame-generation-panel", "uninstall", {"generator": generator})}</button></div></div>}
  {uninstallConfirm&&<ConfirmModal title={tx("ui-frame-generation-panel", "uninstall_2", {"generator": generator})} message={tx("ui-frame-generation-panel", "remove_and_all_versions_installed_by_peligames_game_settings_and_your_lossless_scaling_d", {"generator": generator})} confirmText={tx("ui-frame-generation-panel", "uninstall_component")} isDanger busy={busy} onConfirm={()=>void uninstall()} onCancel={()=>setUninstallConfirm(false)}/>}
  {state&&<>
    {updateAvailable&&!state.update_compatible&&<p className="fg-partition-note">{tx("ui-frame-generation-panel", "version_requires_a_peligames_integration_update_the_current_version_was_preserved", {"state_available_version": state.available_version})}</p>}
    {state.installed_versions.length>0&&<details className="fg-version-history"><summary>{tx("ui-frame-generation-panel", "versions_and_backups")} <small>({state.installed_versions.length})</small></summary><p>{tx("ui-frame-generation-panel", "updates_preserve_older_versions_to_use_a_backup_choose_its_version_in_the_game_s_details")}</p>{state.installed_versions.map(r=><div className="fg-version-row" key={r.version}><span>{r.version}<small>{r.current?(tx("ui-frame-generation-panel", "current")):("Backup")}{r.in_use?tx("ui-frame-generation-panel", "linked_to_profiles"):""}</small></span>{!r.current&&<button type="button" className="modern-mini-btn fg-delete-backup" disabled={blocked||r.in_use} aria-label={tx("ui-frame-generation-panel", "delete_backup", {"r_version": r.version})} title={r.in_use?(tx("ui-frame-generation-panel", "linked_to_profiles_removal_blocked")):(tx("ui-frame-generation-panel", "delete_backup_2"))} onClick={()=>setBackupToDelete(r.version)}><MenuIcon name="trash"/></button>}</div>)}{backupToDelete&&<div className="fg-delete-confirm"><span>{tx("ui-frame-generation-panel", "permanently_delete_backup", {"backuptodelete": backupToDelete})}</span><button type="button" className="modern-mini-btn danger" disabled={blocked} onClick={()=>void removeBackup(backupToDelete)}>{tx("ui-frame-generation-panel", "delete_backup_2")}</button><button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>setBackupToDelete(undefined)}>{tx("ui-frame-generation-panel", "cancel")}</button></div>}</details>}
  </>}
 </section>;
 return <section className="frame-generation-panel fg-game-workflow fg-mako-layout">
  {error&&<p role="alert">{error}</p>}
  {launchSetup&&<MakoLaunchSetup game={launchSetup.game} enabled={launchSetup.enabled} onSaved={onSaved} onClose={()=>setLaunchSetup(undefined)}/>}
  {sandboxOpen&&sandbox&&<MakoLaunchSetup game={game} sandbox={sandbox} onCheckSandbox={checkSandbox} onSaved={onSaved} onClose={()=>setSandboxOpen(false)}/>}
  {game.flatpak&&<div className="fg-sandbox-setup"><span>{sandbox?.ready?(tx("ui-frame-generation-panel", "flatpak_prepared")):(tx("ui-frame-generation-panel", "flatpak_requires_an_extension_and_access_to_the_profile_and_dll"))}</span><button type="button" className="modern-mini-btn" disabled={sandboxBusy||busy||!settings} onClick={()=>void checkSandbox()}>{sandboxBusy?(tx("ui-frame-generation-panel", "checking")):(tx("ui-frame-generation-panel", "prepare_verify_flatpak"))}</button></div>}
  <div className="modern-dlss-panels fg-game-grid">
   <section className="modern-dlss-card fg-game-setup" aria-label={tx("ui-frame-generation-panel", "configure_frame_generation")}>
    <div className="fg-setup-columns"><div className="fg-setup-intro">
    <div className="modern-panel-heading"><h3>{tx("ui-frame-generation-panel", "prepare_for_this_game")}</h3><GameSaveStatusIndicator scope="fg-prepare"/></div>
    <p className="modern-dlss-description">{tx("ui-frame-generation-panel", "check_the_mako_dll_and_set_the_multiplier_changes_save_automatically_for_the_next_launch")}</p>
  <div className="fg-toolbar">
   {state&&settings&&<div className="fg-master-toggle"><span>{tx("ui-frame-generation-panel", "enable_for_this_game")}</span><ModOptionSwitch label={tx("ui-frame-generation-panel", "enable_for_this_game")} enabled={settings.enabled} disabled={startupBlocked||!componentInstalled||!supported} onToggle={()=>patch({enabled:!settings.enabled})}/></div>}
   <span className="fg-generator-name">MAKO</span>
  </div>
  {advanced.afmf?.enabled&&<p className="mod-step-note">{tx("ui-frame-generation-panel", "enabling_mako_automatically_disables_afmf_for_this_game")}</p>}

    {!game.flatpak&&state&&!componentInstalled&&<div className="fg-component-row"><button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>{if(backend==="saved")return;notifyInfo("Frame Generation",tx("ui-frame-generation-panel", "is_not_installed_install_it_in_settings_add_ons", {"mako": "MAKO"}),"frame-mako");openFrameGenerationPreferences(backend);}}>{tx("ui-frame-generation-panel", "install")}</button></div>}
    {state&&state.installed_versions.length>0&&<section className="fg-game-version-history" aria-labelledby={versionsTitleId}><h4 id={versionsTitleId}>{tx("ui-frame-generation-panel", "versions_and_backups")} <small>({state.installed_versions.length})</small></h4><p>{tx("ui-frame-generation-panel", "choose_the_version_for_this_game_updates_preserve_older_versions")}</p><SelectionMenu label={tx("ui-frame-generation-panel", "mako_version_for_this_game")} value={selectedVersion||""} disabled={startupBlocked||!settings} onChange={version=>{if(version!==selectedVersion)patch({runtime_version:version});}} options={state.installed_versions.map(r=>({value:r.version,label:`${r.version} · ${r.current?(tx("ui-frame-generation-panel", "current")):("Backup")}`}))}/></section>}
    </div><div className="fg-setup-source">
    {!state||!settings?<p role="status">{tx("ui-frame-generation-panel", "checking_component_and_profile")}</p>:<div className="fg-source">
  <p>{tx("ui-frame-generation-panel", "requires_lossless_dll_from_the_default_public_steam_version_of_lossless_scaling")}</p>
  <div className="fg-dll"><label>DLL<input value={settings.dll} disabled={startupBlocked} onChange={e=>patch({dll:e.target.value})} placeholder={state.dll||"Lossless.dll"}/></label><button className="modern-mini-btn" disabled={startupBlocked} onClick={()=>void openFilePicker("Lossless Scaling",["dll"]).then(p=>{if(p)patch({dll:p})}).catch(failure)}>{tx("ui-frame-generation-panel", "browse")}</button></div>
   <div className="modern-dlss-setting fg-control-field fg-setting"><span>{settings.adaptive && backend === "mako" ? tx("ui-frame-generation-panel", "maximum_multiplier") : tx("ui-frame-generation-panel", "multiplier")}</span><div className="modern-dlss-segments fg-multipliers" role="group" aria-label={tx("ui-frame-generation-panel", "multiplier")}>{[2,3,4,5].map(n=><button type="button" key={n} aria-pressed={settings.multiplier===n} disabled={blocked||liveUnavailable} onClick={()=>patch({multiplier:n})}>{n}×</button>)}</div></div>
  <dl className="fg-download-info"><div><dt>{tx("ui-frame-generation-panel", "destination")}</dt><dd><button type="button" title={state.installed_versions.some(r=>r.directory===state.install_directory)?state.install_directory:(tx("ui-frame-generation-panel", "folder_available_after_installation"))+state.install_directory} disabled={!state.installed_versions.some(r=>r.directory===state.install_directory)} onClick={()=>void invoke("open_folder",{path:state.install_directory}).catch(failure)}>{state.install_directory}</button></dd></div></dl>
  {externalDll&&<p className="fg-partition-note" role="note">{tx("ui-frame-generation-panel", "dll_on_another_filesystem_if_frame_generation_does_not_work_steam_s_runtime_may_be_block")}</p>}

    {running&&<p role="status">{liveUnavailable?(tx("ui-frame-generation-panel", "mako_is_not_enabled_in_this_profile_close_the_game_to_enable_it")):(tx("ui-frame-generation-panel", "game_running_adjust_multiplier_adaptive_mode_target_fps_flow_scale_and_lighter_model_her"))}</p>}
    <div className="modern-dlss-actions fg-game-actions">
  {!headerStatus&&<span role="status" className="fg-autosave-status">{saveStatus==="saving"?(tx("ui-frame-generation-panel", "saving")):saveStatus==="pending"?(tx("ui-frame-generation-panel", "changes_will_save_automatically")):saveStatus==="error"?(tx("ui-frame-generation-panel", "could_not_save_adjust_the_option_or_retry")):saveStatus==="saved"?(tx("ui-frame-generation-panel", "saved_automatically")):(tx("ui-frame-generation-panel", "changes_save_automatically"))}</span>}
  {saveStatus==="error"&&<button type="button" className="modern-mini-btn" disabled={blocked||liveUnavailable||!supported} onClick={()=>{setSaveStatus("pending");autosave.current.schedule(()=>running?saveLive(settings):save(settings));}}>{tx("ui-frame-generation-panel", "retry")}</button>}
  {game.steam_pending&&<p>{tx("ui-frame-generation-panel", "steam_has_pending_changes_apply_configure_manually_or_revert_beside_the_play_button")}</p>}

    </div>
    </div>}
    </div></div>
   </section>
   <section className="modern-dlss-card fg-game-options" aria-label={tx("ui-frame-generation-panel", "frame_generation_options")}>
    <div className="modern-panel-heading"><h3>{tx("ui-frame-generation-panel", "options")}</h3><GameSaveStatusIndicator scope="fg-options"/></div><p className="modern-dlss-description">{tx("ui-frame-generation-panel", "fine_tune_generation_and_check_compatibility_before_enabling")}</p>
    {state&&settings&&<>
    <MakoAdditionalControls settings={settings} gpus={gpus} gpuError={gpuError} startupBlocked={startupBlocked} liveBlocked={blocked||liveUnavailable} onChange={value=>patch(value,"fg-options")} generationControls={<div className="fg-controls">
   {backend==="mako"&&<div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{tx("ui-frame-generation-panel", "adaptive_mode")}</span><small>{tx("ui-frame-generation-panel", "adjusts_extra_frames_to_the_scene")}</small></span><ModOptionSwitch label={tx("ui-frame-generation-panel", "adaptive_mode")} enabled={settings.adaptive} disabled={blocked||liveUnavailable} onToggle={()=>patch({adaptive:!settings.adaptive},"fg-options")}/></div>}

   {backend==="mako"&&settings.adaptive&&<label className="modern-dlss-setting fg-control-field fg-setting">{tx("ui-frame-generation-panel", "target_output_fps")}<input type="number" min={10} max={1000} value={settings.target_fps} disabled={blocked||liveUnavailable} onChange={e=>patch({target_fps:Number(e.target.value)},"fg-options")}/></label>}
   <label className="modern-dlss-setting fg-control-field fg-setting fg-flow"><span className="fg-setting-copy"><span>{tx("ui-frame-generation-panel", "flow_scale")}</span><small>{tx("ui-frame-generation-panel", "resolution_used_to_estimate_motion")}</small></span><span className="fg-flow-value"><output>{Math.round(settings.flow_scale*100)}%</output><input type="range" aria-label={tx("ui-frame-generation-panel", "flow_scale")} min={25} max={100} value={Math.round(settings.flow_scale*100)} disabled={blocked||liveUnavailable||settings.ultra_performance} onChange={e=>patch({flow_scale:Number(e.target.value)/100},"fg-options")}/></span></label>
   <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{tx("ui-frame-generation-panel", "lighter_model")}</span><small>{tx("ui-frame-generation-panel", "prioritizes_frame_generation_performance")}</small></span><ModOptionSwitch label={tx("ui-frame-generation-panel", "lighter_model")} enabled={settings.performance} disabled={blocked||liveUnavailable||settings.ultra_performance} onToggle={()=>patch({performance:!settings.performance},"fg-options")}/></div>

    </div>} />
  <p className="fg-overlay-support-note">{tx("ui-frame-generation-panel", "configure_the_peligames_overlay_and_background_fps_limit_in_performance_the_overlay_can_")}</p>

  <div className="fg-compatibility"><strong>{tx("ui-frame-generation-panel", "compatibility")}</strong><p>{state.warning}</p><p>{tx("ui-frame-generation-panel", "mangohud_may_count_only_real_frames_higher_output_fps_does_not_proportionally_reduce_inp")}</p>
   {backend==="mako"&&advanced.options.hdr&&<p role="alert">{tx("ui-frame-generation-panel", "disable_hdr_for_this_stable_mako_integration_configured_for_sdr_mako_hdr_is_still_experi")}</p>}
   {game.flatpak&&<p role="alert">{tx("ui-frame-generation-panel", "flatpak_uses_its_own_mako_extension_prepare_verify_flatpak_before_enabling")}</p>}
   {!supported&&<p>{tx("ui-frame-generation-panel", "register_this_installation_in_the_main_library_to_configure_its_launch")}</p>}
   {(advanced.options.mlfg||advanced.options.obs_capture||advanced.options.vkd3d_latency||advanced.options.vulkan_latency)&&<p role="alert">{tx("ui-frame-generation-panel", "disable_ml_frame_generation_in_advanced_options_first")}</p>}

  </div>

    </>}
   </section>
  </div>
 </section>
}

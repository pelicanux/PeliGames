import {useOptiScalerEnvironment} from "./environment";
import {ComponentDownloadProgress,MenuIcon,ModOptionSwitch,useI18n} from "./environment";
import {useEffect,useRef,useState} from "react";
import {createDownloadRateTracker,type TransferProgress} from "../../../_infra/ui-sdk/helpers/downloadProgress";
import {OptiScalerSoftwareStatus} from "./OptiScalerSoftwareStatus";
import type {RuntimeState} from "./environment";
import "./optiscalerPreferences.css";
export function OptiScalerPreferences({disabled,onBusy,embedded=false,onChanged}:{disabled:boolean;onBusy:(value:boolean)=>void;embedded?:boolean;onChanged?:()=>void}){
 const environment=useOptiScalerEnvironment();const api=environment.api;
 const {t}=useI18n();
 const [state,setState]=useState<RuntimeState>();
 const [busy,setBusy]=useState(false);
 const [pendingChannel,setPendingChannel]=useState<RuntimeState["channel"]>();
 const [progress,setProgress]=useState<TransferProgress>();
 const [error,setError]=useState("");
 const [confirmRemoval,setConfirmRemoval]=useState(false);
 const mounted=useRef(false),lock=useRef(false);
 useEffect(()=>{
  mounted.current=true;
  api.runtimeState().then(s=>{if(mounted.current)setState(s)}).catch(e=>{if(mounted.current)setError(String(e))});
  return()=>{mounted.current=false};
 },[]);
 useEffect(()=>{onBusy(busy);return()=>onBusy(false)},[busy,onBusy]);
 const change=async(operation:()=>Promise<RuntimeState>)=>{
  if(lock.current||disabled)return;
  lock.current=true;setBusy(true);setError("");setProgress(undefined);setConfirmRemoval(false);
  try{const next=await operation();if(mounted.current){setState(next);onChanged?.();}}
  catch(e){if(mounted.current){setError(String(e));setProgress(undefined)}}
  finally{lock.current=false;if(mounted.current)setBusy(false)}
 };
 const selectChannel=(channel:RuntimeState["channel"])=>{
  if(!state||disabled||lock.current||state.channel===channel)return;
  setPendingChannel(channel);
  void change(()=>api.setChannel(channel)).finally(()=>{
   if(mounted.current)setPendingChannel(undefined);
  });
 };
 const openDirectory=async()=>{
  if(!state?.directory||disabled||busy)return;
  try{await environment.openDirectory(state.directory);}catch(error){setError(String(error));}
 };
 const removeDownload=()=>void change(()=>api.removeDownload(state!.channel));
 const download=()=>void change(async()=>{
  const rate=createDownloadRateTracker();
  setProgress({phase:"connecting",downloaded:0,total:null});
  await api.download(state!.channel,p=>{if(mounted.current)setProgress(rate(p))});
  const next=await api.runtimeState();
  if(mounted.current)setProgress({phase:"complete",downloaded:0,total:null});
  return next;
 });
 const phaseLabel=t("upscaler",progress?.phase==="complete"?"download_complete":progress?.phase==="verifying"?"download_verifying":progress?.phase==="extracting"?"download_preparing":progress?.phase==="downloading"?"download_transferring":"download_connecting");
 return <section className="mod-workspace" data-preference-id="runtime-optiscaler">
  {!embedded&&<header className="mod-intro"><h3>OptiScaler</h3><p>{t("upscaler","runtime_hint")}</p></header>}
  <section className={embedded?"upscaler-package-settings":"mod-install-surface"}>
   <div className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="graphics"/></span><div><h4>OptiScaler</h4><p>{t("upscaler",state?.channel==="nightly"?"nightly_channel":"stable_channel")}</p></div><span className="mod-status-badge" data-ready={state?.ready||false} role="status">{state?.ready?state.version:t("upscaler",state?"not_installed":"loading")}</span></div>
   <div className="mod-setting-row upscaler-download-row">
    <div className="mod-row-copy"><strong>{t("upscaler","runtime_files")}</strong><p>{t("upscaler",embedded?"package_game_hint":"channel_games_hint")}</p></div>
    <div className="upscaler-download-actions">
     <button type="button" className="mod-action-primary" disabled={disabled||busy||!state} onClick={download}><MenuIcon name="download"/>{t("upscaler",busy?"working":state?.ready?"runtime_update":"download")}</button>
     <div className="upscaler-download-channel" data-saving={pendingChannel!==undefined||undefined} title={t("upscaler","nightly_hint")}>
      <span>{t("upscaler","nightly_label")}</span>
      <ModOptionSwitch appearance="rail" label={t("upscaler","nightly_label")} enabled={(pendingChannel??state?.channel)==="nightly"} disabled={disabled||busy||!state} onToggle={()=>selectChannel(state?.channel==="nightly"?"stable":"nightly")}/>
     </div>
    </div>
   </div>
   {progress&&<ComponentDownloadProgress progress={progress} label={phaseLabel} onDismiss={()=>setProgress(undefined)}/>}
   <div className="mod-setting-row upscaler-package-files">
    <div className="mod-row-copy"><strong>{t("upscaler","package_folder")}</strong><p aria-live="polite">{confirmRemoval?t("upscaler","package_remove_confirm",{channel:t("upscaler",state?.channel==="nightly"?"nightly_channel":"stable_channel")}):t("upscaler","package_remove_hint")}</p></div>
    <div className="upscaler-package-actions" role={confirmRemoval?"group":undefined} aria-label={confirmRemoval?t("upscaler","remove_download"):undefined}>
     {confirmRemoval?<>
      <button type="button" className="modern-mini-btn" onClick={()=>setConfirmRemoval(false)}>{t("upscaler","cancel")}</button>
      <button type="button" className="modern-mini-btn danger" disabled={disabled||busy} onClick={removeDownload}><MenuIcon name="trash"/>{t("upscaler","confirm")}</button>
     </>:<>
      <button type="button" className="modern-mini-btn" disabled={disabled||busy||!state?.directory} onClick={()=>void openDirectory()}><MenuIcon name="folder"/>{t("upscaler","open_package")}</button>
      <button type="button" className="modern-mini-btn danger" disabled={disabled||busy||!state?.directory} onClick={()=>setConfirmRemoval(true)}><MenuIcon name="trash"/>{t("upscaler","remove_download")}</button>
     </>}
    </div>
   </div>
   {!embedded&&<p className="mod-version-label">{t("upscaler","runtime_patcher")}</p>}
   {state?.software&&<OptiScalerSoftwareStatus items={state.software} scope="package"/>}
   {error&&<p role="alert" className="mod-feedback-error">{error}</p>}
  </section>
 </section>;
}

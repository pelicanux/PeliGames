import {useState} from "react";
import type {AddonHost} from "../../../_infra/ui-sdk";
import type {Connection,Runtime} from "./api";

export function RuntimePanel({host,api,state,blocked,run,download}:{host:AddonHost;api:Connection;state?:Runtime;blocked:boolean;run:(work:()=>Promise<Runtime>)=>void;download:()=>void}){
 const [confirmation,setConfirmation]=useState<string>();const t=api.t;
 return <section className="afmf-runtime" aria-label={t("component")}>
  <header><div><h3>{t("component")}</h3><p>{t("shared")}</p></div><span className="mod-status-badge" data-ready={state?.ready}>{state?.ready?state.current:t("missing")}</span></header>
  <div className="afmf-actions"><button type="button" className="mod-action-primary" disabled={blocked||!state?.compatible} onClick={download}><host.ui.Icon name="download"/>{t(state?.ready?"update":"download")}</button><host.ui.Button disabled={blocked} onClick={()=>run(api.check)}><host.ui.Icon name="repair"/>{t("check")}</host.ui.Button></div>
  {state&&<div className="afmf-path"><span>{t("folder")}</span><button type="button" disabled={blocked||!state.ready} title={state.directory} onClick={()=>run(async()=>{await host.files.openDirectory(state.directory);return state})}>{state.directory}</button></div>}
  {Boolean(state?.versions.length)&&<><h4>{t("version")}</h4><div className="afmf-versions">{state!.versions.map(version=><div key={version.version}>
   <span>{version.version}{version.current?` · ${t("installed")}`:""}</span>
   {confirmation===version.version?<><span className="afmf-confirm">{t("confirm_remove")}</span><host.ui.Button disabled={blocked} onClick={()=>setConfirmation(undefined)}>{t("cancel")}</host.ui.Button><host.ui.Button disabled={blocked} onClick={()=>{setConfirmation(undefined);run(()=>api.remove(version.version))}}>{t("confirm")}</host.ui.Button></>:<><host.ui.Button disabled={blocked||version.current||!version.compatible} onClick={()=>run(()=>api.select(version.version))}>{t("select")}</host.ui.Button><host.ui.Button disabled={blocked} onClick={()=>setConfirmation(version.version)}>{t("remove")}</host.ui.Button></>}
  </div>)}</div></>}
 </section>;
}

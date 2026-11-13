import {useState} from "react";
import type {AddonHost} from "../../../_infra/ui-sdk";
import type {Api,Runtime} from "./api";
export function RuntimePanel({host,api,state,blocked,run}:{host:AddonHost;api:Api;state?:Runtime;blocked:boolean;run:(work:()=>Promise<unknown>)=>void}){
 const [remove,setRemove]=useState(false),[latest,setLatest]=useState("");const t=api.t;
 const pick=(method:string,directory=false)=>run(async()=>{
  const path=directory?await host.files.chooseDirectory?.(t("import_backend")):await host.files.chooseFile?.(t(method==="model.extract"?"extract":"import_models"),method==="model.extract"?["dll","zip"]:["bin"]);
  if(path)await api.job(method,{path});
 });
 return <section className="mochi-runtime">
  <header><div><h3>DLSSNR-AMD · Mochizuki</h3><p>{t("shared")}</p></div><span className="mod-status-badge" data-ready={state?.ready}>{state?.current||t("missing")}</span></header>
  <div className="mochi-actions"><host.ui.Button primary disabled={blocked} onClick={()=>run(()=>api.job("runtime.download"))}><host.ui.Icon name="download"/>{t(state?.ready?"update":"download")}</host.ui.Button><host.ui.Button disabled={blocked} onClick={()=>run(async()=>{const version=await api.job<string>("runtime.check");if(version)setLatest(version)})}>{t("check")}</host.ui.Button><host.ui.Button disabled={blocked} onClick={()=>pick("runtime.import",true)}>{t("import_backend")}</host.ui.Button></div>
  {latest&&<p>{t("latest")}: {latest}</p>}
  <host.ui.Path compact label={t("component_folder")} path={state?.directory}/>
  <div className="mochi-model"><h4>{t("model")}</h4><p>{t("model_hint")}</p><span className="mod-status-badge" data-ready={state?.models}>{t(state?.models?"model_ready":"model_missing")}</span><div className="mochi-actions"><host.ui.Button disabled={blocked||!state?.ready} onClick={()=>pick("model.extract")}>{t("extract")}</host.ui.Button><host.ui.Button disabled={blocked||!state?.ready} onClick={()=>pick("model.import")}>{t("import_models")}</host.ui.Button></div></div>
  {state?.ready&&<div className="mochi-actions mochi-removal">{remove?<><span>{t("remove_hint")}</span><host.ui.Button disabled={blocked} onClick={()=>setRemove(false)}>{t("cancel")}</host.ui.Button><host.ui.Button disabled={blocked} onClick={()=>{setRemove(false);run(()=>api.call("runtime.remove"))}}>{t("confirm")}</host.ui.Button></>:<host.ui.Button disabled={blocked} onClick={()=>setRemove(true)}>{t("remove_component")}</host.ui.Button>}</div>}
 </section>;
}

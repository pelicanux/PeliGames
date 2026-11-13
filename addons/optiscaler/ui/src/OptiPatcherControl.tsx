import {useOptiScalerEnvironment} from "./environment";
import {ModOptionSwitch,useI18n} from "./environment";
import {useState} from "react";
import type {UpscalerState} from "./settings";
export function OptiPatcherControl({directory,state,disabled,onBusy,onChanged}:{directory:string;state:UpscalerState["patcher"];disabled:boolean;onBusy:(value:boolean)=>void;onChanged:()=>void}){
 const environment=useOptiScalerEnvironment();const api=environment.api;
 const {t}=useI18n();const [busy,setBusy]=useState(false);const [error,setError]=useState("");
 const external=!!state?.installed&&!state.managed;
 const toggle=async()=>{if(busy||disabled||external)return;setBusy(true);onBusy(true);setError("");try{await api.setPatcher(directory,!state?.installed);onChanged();}catch(e){setError(String(e));}finally{setBusy(false);onBusy(false);}};
 return <div className="upscaler-patcher"><div className="upscaler-patcher-row"><div><h3>OptiPatcher</h3><small>{t("upscaler",external?"patcher_external":busy?"patcher_working":state?.installed&&!state.enabled?"patcher_loader_off":"patcher_hint")}</small></div><ModOptionSwitch appearance="rail" label="OptiPatcher" enabled={!!state?.installed} disabled={disabled||busy||external} onToggle={()=>void toggle()}/></div>{error&&<p role="alert" className="neural-startup-error">{error}</p>}</div>;
}

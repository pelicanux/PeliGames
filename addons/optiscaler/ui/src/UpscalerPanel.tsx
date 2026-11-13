import type {OptiScalerMessageKey} from "./messages";
import {useOptiScalerEnvironment} from "./environment";
import {AddonProjectLinks,GamePathTile,MenuIcon,SelectionMenu,useI18n} from "./environment";
import {OptiScalerPreferences} from "./OptiScalerPreferences";
import {OptiScalerUsage} from "./OptiScalerUsage";
import {OptiScalerSoftwareStatus} from "./OptiScalerSoftwareStatus";
import proxyDlls from "../../core/proxies.json";
import {UpscalerControls} from "./UpscalerControls";
export {UpscalerControls} from "./UpscalerControls";
import {optiscalerMenuKeyLabel} from "./menuShortcut";
import {useEffect,useState} from "react";
import type {GameReference} from "./environment";
import {createUpscalerChoice} from "./choice";
import {upscalerFields,type UpscalerState} from "./settings";
import "./upscaler.css";
import {OptiPatcherControl} from "./OptiPatcherControl";
interface Props {directory:string;game:GameReference;blocked:boolean;onManageNr:()=>void;onRefresh:()=>Promise<void>;onBusy:(busy:boolean)=>void;onShortcut:(key:string)=>Promise<void>}
export function UpscalerPanel(props:Props){
 const environment=useOptiScalerEnvironment();const api=environment.api;
 const {t}=useI18n();const tx=(key:OptiScalerMessageKey)=>t("upscaler",key);
 const [state,setState]=useState<UpscalerState>();const [error,setError]=useState("");const [operation,setOperation]=useState(false);const [revision,setRevision]=useState(0);const [confirm,setConfirm]=useState(false);const [launchLine,setLaunchLine]=useState<string>();
 const [choice,setChoice]=useState<ReturnType<typeof createUpscalerChoice>>();const [controlsBusy,setControlsBusy]=useState(false);const [runtimeBusy,setRuntimeBusy]=useState(false);
 useEffect(()=>{setState(undefined);setChoice(undefined);setConfirm(false);},[props.directory]);
 useEffect(()=>{let active=true;
  api.inspect(props.directory).then(next=>{if(!active)return;setState(next);
   const directory=props.directory;const onShortcut=props.onShortcut;
   const initial={...next.settings};
   setChoice(createUpscalerChoice(Object.fromEntries(Object.entries(initial).filter(([key])=>(key!=="FrameGen.FGNvngxReplacement"||next.framegen_modern!==false)&&upscalerFields.some(field=>field.key===key&&(field.group!=="framegeneration"||next.framegen_available)))),next.installed?async(key,value)=>{
    await api.update(directory,key,value);
    if(key==="Menu.ShortcutKey")await onShortcut(optiscalerMenuKeyLabel(value)).catch(error=>setError(String(error)));
   }:undefined));
  }).catch(e=>{if(active)setError(String(e));});return()=>{active=false};
 },[props.directory,revision]);
 useEffect(()=>{props.onBusy(operation||controlsBusy||runtimeBusy);return()=>props.onBusy(false)},[operation,controlsBusy,runtimeBusy,props.onBusy]);
 const act=async(action:"install"|"update"|"remove"="install")=>{
  if(!choice||!state||state.nr_route||operation||controlsBusy||runtimeBusy||props.blocked)return;setOperation(true);setError("");setConfirm(false);
  const {directory,game,onRefresh}=props;try{
   const remove=action==="remove";
   const line=remove?null:action==="update"?await api.upgrade(directory):await api.install(directory,proxy,{});
   if(remove)await api.remove(directory);
   else {
    const installed=await api.inspect(directory);
    await props.onShortcut(optiscalerMenuKeyLabel(installed.settings["Menu.ShortcutKey"])).catch(error=>setError(String(error)));
   }
   setLaunchLine(line??undefined);
   try{const updated=await environment.synchronizeLaunch(game,directory,line);if(updated)setLaunchLine(undefined);}catch(e){setError(`${tx("profile_error")} ${String(e)}`);}
   await onRefresh();setRevision(v=>v+1);
  }catch(e){setError(String(e));}finally{setOperation(false);}
 };
 const [proxy,setProxy]=useState("dxgi.dll");
 useEffect(()=>{setError("");setLaunchLine(undefined)},[props.directory]);
 return <div className="upscaler-panel">
  {error&&<p role="alert" className="neural-startup-error">{error}</p>}
  {!state&&!error&&<p role="status">{tx("loading")}</p>}
  {!state&&error&&<button type="button" className="modern-mini-btn" onClick={()=>setRevision(v=>v+1)}>{tx("retry")}</button>}
  {state&&<>
   {choice&&<UpscalerControls key={props.directory} choice={choice} installed={state.installed} framegenAvailable={state.framegen_available} framegenModern={state.framegen_modern!==false} blocked={operation||runtimeBusy||props.blocked||!state.installed} configurationAvailable={state.installed&&(!state.nr_route||state.nr_route==="optiscaler")} onBusy={setControlsBusy}
    management={<div className="upscaler-management">
   <section className="upscaler-installation" aria-label="OptiScaler">
    <div className="upscaler-summary-row">
     <div className="upscaler-summary-heading"><h3>OptiScaler</h3><span>{tx(state.installed?"installed":"not_installed")}{state.installed&&state.proxy?` · ${state.proxy}`:""}</span></div>
     <div className="upscaler-summary-actions">
      {!state.installed&&!state.managed&&!state.nr_route&&<>
       <SelectionMenu label={tx("proxy")} value={proxy} options={[...proxyDlls.map(value=>({value,label:value})),{value:"OptiScaler.asi",label:tx("proxy_asi"),disabled:true}]} appearance="compact" onChange={setProxy} disabled={operation||controlsBusy||runtimeBusy||props.blocked}/>
       <button type="button" className="modern-dlss-primary" disabled={!state.component_ready||operation||controlsBusy||runtimeBusy||props.blocked} onClick={()=>void act()}><MenuIcon name="download"/>{tx(operation?"working":"install")}</button>
      </>}
      {state.managed&&!state.nr_route&&<><button type="button" className="modern-mini-btn" disabled={!state.component_ready||operation||controlsBusy||runtimeBusy||props.blocked} onClick={()=>void act("update")}><MenuIcon name="repair"/>{tx(operation?"game_updating":"game_update")}</button><button type="button" className="modern-mini-btn danger" disabled={operation||controlsBusy||runtimeBusy||props.blocked} onClick={()=>setConfirm(true)}><MenuIcon name="trash"/>{tx("remove")}</button></>}
     </div>
    </div>
    {!state.installed&&!state.nr_route&&<small className="upscaler-summary-note">{tx("proxy_hint")}</small>}
    {!state.installed&&<small className="upscaler-summary-note">{tx("configuration_locked")}</small>}
    <GamePathTile compact label={tx("destination")} path={props.directory}/>
    {state.nr_route?<div className="upscaler-conflict" role="note"><MenuIcon name="info"/><p>{tx(state.nr_route==="optiscaler"?"nr_optiscaler":state.nr_route==="reshade"?"nr_reshade":"nr_unknown")}</p><button type="button" className="modern-mini-btn" disabled={operation||controlsBusy||runtimeBusy} onClick={()=>{try{props.onManageNr()}catch(error){setError(String(error))}}}>{tx("manage_nr")}</button></div>:<p className="upscaler-summary-note">{tx(state.installed?"intro":"install_hint")}</p>}
    {confirm&&<div className="upscaler-confirm"><p>{tx("remove_confirm")}</p><button type="button" className="modern-mini-btn danger" onClick={()=>void act("remove")}>{tx("remove")}</button><button type="button" className="modern-mini-btn" onClick={()=>setConfirm(false)}>{tx("cancel")}</button></div>}
    {launchLine&&<code className="upscaler-launch-line">{launchLine}</code>}
   </section>
   {state.installed&&state.software&&<OptiScalerSoftwareStatus items={state.software} scope="game"/>}
   <OptiScalerPreferences embedded disabled={operation||controlsBusy||props.blocked} onBusy={setRuntimeBusy} onChanged={()=>setRevision(v=>v+1)}/>
   <AddonProjectLinks addon="optiscaler"/>
    </div>}
    compatibility={state.installed?<OptiPatcherControl directory={props.directory} state={state.patcher} disabled={operation||controlsBusy||runtimeBusy||props.blocked} onBusy={setControlsBusy} onChanged={()=>setRevision(v=>v+1)}/>:undefined}
    usage={<OptiScalerUsage game={props.game} installed={state.installed} blocked={operation||controlsBusy||runtimeBusy||props.blocked}/>}/>}
  </>}
 </div>;
}

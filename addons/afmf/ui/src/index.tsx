import {useEffect,useRef,useState} from "react";
import type {AddonHost,AddonInstance,AddonLaunchState,AddonViewProps} from "../../../_infra/ui-sdk";
import type {TransferProgress} from "../../../_infra/ui-sdk/helpers/downloadProgress";
import {createDownloadRateTracker} from "../../../_infra/ui-sdk/helpers/downloadProgress";
import {createLatestWriteQueue} from "../../../_infra/ui-sdk/helpers/latestWriteQueue";
import {connect,type Runtime,type Settings,type Connection,type Message} from "./api";
import {RuntimePanel} from "./RuntimePanel";
import {Controls} from "./Controls";
import "./afmf.css";

const projects=[{name:"AFMF Linux",url:"https://github.com/serialexperimentslainnnn/afmf-linux"}];
function useRuntime(host:AddonHost,api:Connection){
 const [state,setState]=useState<Runtime>(),[busy,setBusy]=useState(false),[error,setError]=useState(""),[progress,setProgress]=useState<TransferProgress>();
 const locked=useRef(false),rate=useRef(createDownloadRateTracker());
 const run=(work:()=>Promise<Runtime>)=>{
  if(locked.current)return;locked.current=true;setBusy(true);host.setBusy(true);setError("");
  void work().then(value=>{if(!host.lifecycle.signal.aborted)setState(value)}).catch(async error=>{if(host.lifecycle.signal.aborted)return;setError(String(error));try{const value=await api.state();if(!host.lifecycle.signal.aborted)setState(value)}catch{/* Preserve the original operation error. */}}).finally(()=>{
   locked.current=false;if(!host.lifecycle.signal.aborted){setBusy(false);host.setBusy(false);setProgress(undefined);}
  });
 };
 useEffect(()=>{run(api.state)},[]);
 const download=()=>{if(!state)return;rate.current=createDownloadRateTracker();run(()=>api.download(state.latest,value=>{if(!host.lifecycle.signal.aborted)setProgress(rate.current(value))}))};
 return {state,busy,error,progress,run,download};
}
function Status({host,api,operation}:{host:AddonHost;api:Connection;operation:ReturnType<typeof useRuntime>}){
 return <>{operation.progress&&<host.ui.Progress progress={operation.progress} label={api.t("download_progress")}/>}{operation.error&&<p className="mod-feedback-error" role="alert">{operation.error}</p>}</>;
}
export function createAddon(host:AddonHost):AddonInstance {
 const api=connect(host),t=api.t;
 function Preferences({disabled}:AddonViewProps){
  const operation=useRuntime(host,api);
  return <div className="afmf-addon mod-workspace"><RuntimePanel host={host} api={api} {...operation} blocked={disabled||operation.busy}/><Status host={host} api={api} operation={operation}/><host.ui.Projects projects={projects}/></div>;
 }
 function Game({context,disabled}:AddonViewProps){
  const [category,setCategory]=useState("prepare"),[settings,setSettings]=useState<Settings>(),[launch,setLaunch]=useState<AddonLaunchState>(),[support,setSupport]=useState<{available:boolean;code:Message}>(),[saving,setSaving]=useState(false),[saved,setSaved]=useState(false),[error,setError]=useState("");
  const operation=useRuntime(host,api),latest=useRef<Settings|undefined>(undefined);
  const game=context.game;
  const [queue]=useState(()=>createLatestWriteQueue<Settings,void>(async next=>{
   if(!game||!host.game?.launch)throw new Error(t("bridge_missing"));
   const checked=await api.validate(next);await host.game.launch.save(game.id,checked);latest.current=checked;
  },(next,_result,error)=>{
   if(host.lifecycle.signal.aborted)return;setSaving(false);host.setBusy(false);
   if(error){setError(String(error));setSettings(latest.current);}else{setSettings(next);setSaved(true);setError("");}
  }));
  useEffect(()=>()=>queue.dispose(),[queue]);
  useEffect(()=>{
   if(!game||!host.game?.launch){setError(t("bridge_missing"));return;}
   let live=true;
   void Promise.all([api.defaults(),host.game.launch.read(game.id)]).then(async([defaults,launch])=>{
    const settings={...defaults,...(launch.settings&&typeof launch.settings==="object"?launch.settings:{})};
    const support=await api.support({...launch.executable,flatpak:launch.flatpak});
    if(live&&!host.lifecycle.signal.aborted){latest.current=settings;setSettings(settings);setLaunch(launch);setSupport(support);}
   }).catch(error=>{if(live&&!host.lifecycle.signal.aborted)setError(String(error))});
   return()=>{live=false};
  },[game?.id]);
  const blocked=disabled||operation.busy||saving||Boolean(game?.running||launch?.running);
  const patch=(value:Partial<Settings>)=>{if(!settings||blocked)return;const next={...settings,...value};setSettings(next);setSaving(true);setSaved(false);setError("");host.setBusy(true);queue.enqueue(next)};
  const compatible=Boolean(operation.state?.ready&&launch?.supported&&support?.available&&!launch?.options.some(option=>["mlfg","optiscaler"].includes(option)));
  const management=<RuntimePanel host={host} api={api} {...operation} blocked={blocked}/>;
  return <div className="afmf-addon mod-workspace">
   <host.ui.Categories label="AFMF" active={category} onChange={setCategory} categories={[
    {id:"management",label:t("manage"),icon:"download",content:management},
    {id:"prepare",label:t("prepare"),icon:"settings",content:<>
     <h3>{t("enable")}</h3><p>{t("description")}</p>
     {settings?<div className="afmf-field"><span>{t("enable")}<small>{t("next_launch")}</small></span><host.ui.Switch label={t("enable")} enabled={settings.enabled} disabled={blocked||(!settings.enabled&&!compatible)} onToggle={()=>patch({enabled:!settings.enabled})}/></div>:<p role="status">{t("loading")}</p>}
     {!operation.state?.ready&&<p>{t("install_first")}</p>}
     {support&&<p className="afmf-hint">{t(support.code)}</p>}
     {launch&&!launch.supported&&<p>{t("unsupported_entry")}</p>}
     {launch?.options.some(option=>["mlfg","optiscaler"].includes(option))&&<p role="alert">{t("conflict")}</p>}
     {launch?.competing&&<p className="afmf-hint">{t("exclusive")}</p>}
     {(game?.running||launch?.running)&&<p role="status">{t("running")}</p>}
    </>},
    {id:"adjust",label:t("adjust"),icon:"sliders",content:<><h3>{t("adjust")}</h3>{settings&&<Controls host={host} api={api} settings={settings} blocked={blocked} patch={patch}/>}<p className="afmf-hint">{t("compatibility")}</p>{launch?.gamescope&&<p className="afmf-hint">{t("gamescope")}</p>}</>},
    {id:"how",label:t("how"),icon:"info",content:<><h3>{t("how_title")}</h3><p>{t("how_text")}</p>{game&&<host.ui.LaunchProfile gameId={game.id} inline disabled={blocked}/>}</>},
   ]}/>
   {saving&&<p className="afmf-hint" role="status">{t("saving")}</p>}{saved&&!saving&&<p className="afmf-hint" role="status">{t("saved")}</p>}
   <Status host={host} api={api} operation={operation}/>{error&&<p role="alert" className="mod-feedback-error">{error}</p>}
  </div>;
 }
 function Overview({context}:AddonViewProps){
  const [enabled,setEnabled]=useState(false);
  useEffect(()=>{let live=true;if(context.game&&host.game?.launch)void host.game.launch.read(context.game.id).then(state=>{if(live&&!host.lifecycle.signal.aborted)setEnabled(Boolean((state.settings as {enabled?:boolean}|null)?.enabled))}).catch(()=>{});return()=>{live=false}},[context.game?.id]);
  return enabled?<div className="afmf-addon afmf-overview"><strong>AFMF · 2×</strong><span>{t("overview_hint")}</span></div>:null;
 }
 return {apiVersion:1,id:host.addon.id,version:host.addon.version,views:{preferences:Preferences,game:Game,overview:Overview},dispose(){}};
}

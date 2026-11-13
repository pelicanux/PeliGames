import {useEffect,useRef,useState} from "react";
import type {AddonHost,AddonInstance,AddonViewProps} from "../../../_infra/ui-sdk";
import type {TransferProgress} from "../../../_infra/ui-sdk/helpers/downloadProgress";
import {connect,type Runtime,type GameState} from "./api";
import {RuntimePanel} from "./RuntimePanel";
import {launchOptionsFromInstallOutput,steamLaunchOptionsForRoute,defaultShortcutForRoute} from "./launchOptions";
import "./mochi.css";
const projects=[{name:"Mochizuki · DLSSNR-AMD",url:"https://gitlab.com/mochizuki0323/DLSSNR-AMD"}];
export function createAddon(host:AddonHost):AddonInstance{
 const api=connect(host),t=api.t;
 function Panel({context,disabled}:AddonViewProps){
  const game=context.game,dir=game?.executableDirectory;
  const [page,setPage]=useState(context.target==="settings"?"manage":"prepare"),[runtime,setRuntime]=useState<Runtime>(),[current,setCurrent]=useState<GameState>();
  const [bits,setBits]=useState("64"),[route,setRoute]=useState("optiscaler"),[model,setModel]=useState("small"),[int4,setInt4]=useState(false),[lumenite,setLumenite]=useState(false),[enabled,setEnabled]=useState(true),[renodx,setRenodx]=useState(false),[level,setLevel]=useState("3"),[shortcut,setShortcut]=useState("Insert");
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[log,setLog]=useState(""),[progress,setProgress]=useState<TransferProgress>(),[remove,setRemove]=useState(false);
  const loaded=useRef(""),locked=useRef(false),alive=()=>!host.lifecycle.signal.aborted;
  const refresh=async()=>{
   const runtime=await api.state();if(!alive())return;setRuntime(runtime);
   if(dir){const value=await api.inspect(dir,bits);if(!alive())return;setCurrent(value);if(value.settings){setEnabled(value.settings.enabled);setRenodx(value.settings.renodx);}if(value.optimized_layers!==null)setLevel(String(value.optimized_layers));
    const key=`${dir}:${bits}`;if(loaded.current!==key){loaded.current=key;if(value.route)setRoute(value.route);if(value.shortcut)setShortcut(value.shortcut);setLumenite(Boolean(value.lumenite_selected));
     const network=value.selected_network||"";setInt4(network.includes("OPTIMIZED"));setModel(network.includes("EXPERIMENTAL")?"experimental":network.includes("SMALL")?"small":value.small?"small":"original");
    }}
  };
  useEffect(()=>{void refresh().catch(error=>{if(alive())setError(String(error))})},[dir,bits]);
  useEffect(()=>{if(!game||!host.game?.launch)return;void host.game.launch.read(game.id).then(value=>{if(alive()&&value.executable.bits)setBits(String(value.executable.bits))}).catch(error=>{if(alive())setError(String(error))})},[game?.id]);
  const run=(work:()=>Promise<unknown>)=>{if(locked.current||disabled)return;locked.current=true;setBusy(true);setError("");setProgress(undefined);host.setBusy(true);
   void work().then(refresh).catch(error=>{if(alive()){setError(String(error));setLog(String(error));}return refresh().catch(()=>{})}).finally(()=>{locked.current=false;if(alive()){setBusy(false);setProgress(undefined);host.setBusy(false)}});
  };
  const blocked=disabled||busy||Boolean(game?.running);
  const mutate=async(action:Record<string,unknown>)=>{
   if(!dir||!game)throw Error(t("choose_destination"));
   const output=await api.job<string>("game.mutate",{directory:dir,bits,action},p=>{if(alive())setProgress(p)});if(!alive())return;
   if(action.operation==="install"||action.operation==="remove"){
    const line=action.operation==="remove"?null:launchOptionsFromInstallOutput(output||"")||steamLaunchOptionsForRoute(route);
    setLog(output||"");await host.game?.synchronizeLaunch(game.id,dir,line);await host.game?.refresh(game.id);
   }
  };
  if(game?.native)return <p>{t("native")}</p>;
  if(context.target==="game-overview")return current?.installation.dlssnr_installed?<p className="mochi-overview"><strong>DLSSNR · Mochizuki</strong><span>{current.installation.status} · {t(current.settings?.enabled?"enabled":"disabled")}</span></p>:null;
  const choice=(label:string,value:string,options:{value:string;label:string;disabled?:boolean}[],change:(value:string)=>void)=><host.ui.Choice label={label} value={value} options={options} appearance="compact" disabled={blocked} onChange={change}/>;
  const options=(values:string[])=>values.map(value=>({value,label:value}));
  const manage=<RuntimePanel host={host} api={api} state={runtime} blocked={blocked} run={run}/>;
  const prepare=<section className="mochi-prepare"><h3>{t("prepare")}</h3><p>{t("prepare_hint")}</p>
   {!dir?<><p>{t("choose_destination")}</p><host.ui.Button disabled={blocked} onClick={()=>run(async()=>{if(game)await host.game?.chooseExecutableDirectory(game.id)})}>{t("choose_folder")}</host.ui.Button></>:<>
    <host.ui.Path compact label={t("destination")} path={dir}/>
    {!runtime?.ready||!runtime.models||!current?.original?<><p>{t("complete_setup")}</p><host.ui.Button disabled={blocked} onClick={()=>setPage("manage")}>{t("manage")}</host.ui.Button></>:null}
    <div className="mochi-grid"><label>{t("route")}{choice(t("route"),route,options(["optiscaler","reshade","vulkan","dx9"]),value=>{setRoute(value);setShortcut(defaultShortcutForRoute(value))})}</label><label>{t("architecture")}{choice(t("architecture"),bits,options(["64","32"]),setBits)}</label><label>{t("network")}{choice(t("network"),int4?"optimized":model,[{value:"original",label:t("original")},{value:"small",label:t("new"),disabled:!current?.small},{value:"experimental",label:t("experimental"),disabled:!current?.experimental},{value:"optimized",label:t("optimized"),disabled:!current?.optimized}],value=>{setInt4(value==="optimized");setModel(value==="optimized"?"original":value)})}</label></div>
    {int4&&<label>{t("quality")}{choice(t("quality"),level,["quality","balanced","performance","ultra"].map((key,index)=>({value:String(index),label:t(key as "quality")})),value=>{setLevel(value);if(current?.optimized_layers!==null&&current?.optimized_layers!==undefined)run(()=>mutate({operation:"optimized",level:Number(value)}))})}</label>}
    {route!=="optiscaler"&&current?.lumenite&&<div className="mochi-toggle"><span>Lumenite</span><host.ui.Switch label="Lumenite" enabled={lumenite} disabled={blocked} onToggle={()=>setLumenite(!lumenite)}/></div>}
    <div className="mochi-actions"><host.ui.Button primary disabled={blocked||!runtime?.models||!current?.ready||!current.original||bits==="32"&&route==="optiscaler"} onClick={()=>run(()=>mutate({operation:"install",route,model,int4,lumenite,renodx,enabled,shortcut,optimized_layers:int4?Number(level):null}))}>{t(current?.installation.dlssnr_installed?"repair":"install")}</host.ui.Button>
    {current?.installation.dlssnr_installed&&(remove?<><span>{t("remove_game_hint")}</span><host.ui.Button disabled={blocked} onClick={()=>setRemove(false)}>{t("cancel")}</host.ui.Button><host.ui.Button disabled={blocked} onClick={()=>{setRemove(false);run(()=>mutate({operation:"remove"}))}}>{t("confirm")}</host.ui.Button></>:<host.ui.Button disabled={blocked} onClick={()=>setRemove(true)}>{t("uninstall")}</host.ui.Button>)}</div><small>{t("repair_hint")}</small>
   </>}
  </section>;
  const settings=<section><h3>{t("settings")}</h3><p>{t("settings_hint")}</p>
   <div className="mochi-toggle"><span>{t("start_enabled")}</span><host.ui.Switch label={t("start_enabled")} enabled={enabled} disabled={blocked||!dir} onToggle={()=>{if(current?.installation.dlssnr_installed)run(()=>mutate({operation:"startup",enabled:!enabled,renodx}));else setEnabled(!enabled)}}/></div>
   <div className="mochi-toggle"><span>RenoDX</span><host.ui.Switch label="RenoDX" enabled={renodx} disabled={blocked||!dir} onToggle={()=>{if(current?.installation.dlssnr_installed)run(()=>mutate({operation:"startup",enabled,renodx:!renodx}));else setRenodx(!renodx)}}/></div>
   <label>{t("shortcut")}{choice(t("shortcut"),shortcut,options(["Insert","Home","End","Page Up","Page Down","F1","F2","F11","F12"]),value=>{setShortcut(value);if(current?.installation.dlssnr_installed)run(()=>mutate({operation:"shortcut",key:value,route}))})}</label><p>{t("nr_menu")}</p>
  </section>;
  const usage=<section><h3>{t("how_to")}</h3><p>{t("usage")}</p>{game&&<host.ui.LaunchProfile gameId={game.id} inline disabled={blocked}/>}</section>;
  return <div className="mochi-addon"><host.ui.Categories label="DLSSNR · Mochizuki" active={page} onChange={setPage} categories={[
   {id:"manage",label:t("manage"),icon:"download",content:manage},
   ...(game?[{id:"prepare",label:t("prepare"),icon:"settings" as const,content:prepare},{id:"settings",label:t("settings"),icon:"sliders" as const,content:settings},{id:"usage",label:t("how_to"),icon:"info" as const,content:usage}]:[]),
  ]}/>{progress&&<host.ui.Progress progress={progress} label={t("working")}/>}{error&&<p role="alert">{error}</p>}{log&&<details><summary>{t("log")}</summary><pre>{log}</pre></details>}<host.ui.Projects projects={projects}/></div>;
 }
 return {apiVersion:1,id:host.addon.id,version:host.addon.version,views:{preferences:Panel,game:Panel,overview:Panel},dispose(){}};
}

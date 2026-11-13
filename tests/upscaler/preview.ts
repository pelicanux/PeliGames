import {defaultUpscalerSettings, type OptiScalerSoftware, type UpscalerState} from "../../addons/optiscaler/ui/src/settings";
export function createUpscalerPreview(neural:(path:string)=>boolean){
 let channel="stable";const packages:Record<string,string>={stable:"v0.9.4"};
 const software=(present:boolean,version:string|null,patcher=false):OptiScalerSoftware[]=>["optiscaler","dlss","fsr","xess","fakenvapi","streamline","optipatcher"].map(id=>({id:id as OptiScalerSoftware["id"],present:id==="optipatcher"?patcher:present&&["optiscaler","fsr","xess"].includes(id),integrated:present&&id==="fakenvapi",version:id==="optiscaler"?version:null}));
 const runtimeState=()=>({channel,version:packages[channel]??null,ready:!!packages[channel],directory:packages[channel]?`/preview/components/OptiScaler/${channel==="stable"?"official":"nightly"}`:null,software:software(!!packages[channel],packages[channel]??null)});
 const data=new Map<string,UpscalerState>();
 const state=(path:string):UpscalerState=>{
  if(!data.has(path)){const shared=neural(path);data.set(path,{installed:shared,managed:false,nr_route:shared?"optiscaler":null,component_ready:true,framegen_available:true,framegen_modern:true,patcher:{installed:false,enabled:false,managed:false},proxy:shared?"dxgi.dll":null,settings:defaultUpscalerSettings()});}
  const stored=data.get(path)!;
  if(!stored.software)stored.software=software(stored.installed,null,stored.patcher?.installed);
  const next=structuredClone(stored);
  if(!next.installed){next.component_ready=!!packages[channel];next.framegen_modern=channel==="nightly";}
  return next;
 };
 return {runtimeState,removeDownload:(selected:string)=>{if(selected!==channel)throw Error("The OptiScaler channel changed");delete packages[channel];return runtimeState()},setChannel:(value:string)=>{if(value!=="stable"&&value!=="nightly")throw Error("Invalid channel");channel=value;return runtimeState()},download:()=>{packages[channel]=channel==="nightly"?"10.0.0-pre1":"v0.9.4"},state,patcher:(path:string,enabled:boolean)=>{state(path);data.get(path)!.patcher={installed:enabled,enabled,managed:enabled};const item=data.get(path)!.software?.find(item=>item.id==="optipatcher");if(item)item.present=enabled;},update:(path:string,key:string,value:string)=>{state(path);const next=data.get(path)!;next.settings[key]=value;return structuredClone(next.settings);},
 install:(path:string,proxy:string,settings:Record<string,string>)=>{if(!packages[channel])throw Error("Prepare OptiScaler first");const next=state(path);data.set(path,{...next,installed:true,managed:true,nr_route:null,proxy,software:software(true,packages[channel]),framegen_modern:channel==="nightly",settings:{...defaultUpscalerSettings(channel==="nightly"),...settings}});return `WINEDLLOVERRIDES="${proxy.replace(/\.dll$/,'')}=n,b" %command%`;},
 upgrade:(path:string)=>{const next=state(path);if(!next.managed||next.nr_route)throw Error("Instalação não gerenciada");if(!packages[channel])throw Error("Prepare OptiScaler first");data.set(path,{...next,software:software(true,packages[channel],next.patcher?.installed),framegen_modern:channel==="nightly",settings:{...defaultUpscalerSettings(channel==="nightly"),...Object.fromEntries(Object.entries(next.settings).filter(([key])=>!key.startsWith("FSR.Fsr4")))}});return `WINEDLLOVERRIDES="${next.proxy!.replace(/\.dll$/,'')}=n,b" %command%`;},
 remove:(path:string)=>{data.delete(path);},
 };
}

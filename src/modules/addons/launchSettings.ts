import type {GameInfo} from "../../components/GameGrid";
import {normalizeAdvanced,isFrameGenerationWrapper,type AdvancedSettings} from "../../services/advancedSettings";
import type {AddonCategory,AddonOverview} from "./contracts";

export function addonLaunchData(game:GameInfo,id:string):unknown {
 const settings=normalizeAdvanced(game.advanced);
 const own=settings.addon_launch?.[id];
 return own?{...own.data,enabled:own.enabled}:settings[id]??null;
}
/** Activation is host metadata; disabling preserves the module's tuning payload. */
export function releaseAddonLaunchGroup(settings:AdvancedSettings,group:AddonCategory,overview?:AddonOverview):AdvancedSettings {
 const modules={...settings.addon_launch};
 const next:AdvancedSettings={...settings,addon_launch:modules};
 for(const [id,value] of Object.entries(modules))if(value.group===group)modules[id]={...value,enabled:false};
 for(const [id,entry] of Object.entries(overview?.state.addons??{})){
  const manifest=entry.versions[entry.selected??""]?.manifest,legacy=next[id];
  if(manifest?.category===group&&legacy&&typeof legacy==="object")next[id]={...legacy,enabled:false};
 }
 return next;
}
export function activeFrameGenerationAddons(settings:AdvancedSettings|undefined,overview:AddonOverview|undefined){
 return Object.entries(overview?.state.addons??{}).flatMap(([id,installed])=>{
  const manifest=installed.versions[installed.selected??""]?.manifest;
  const own=settings?.addon_launch?.[id];
  const legacy=settings?.[id] as {enabled?:boolean}|undefined;
  return installed.enabled&&manifest?.category==="frame-generation"&&(own?own.enabled:legacy?.enabled)?[{id,name:manifest.name}]:[];
 });
}
/** Only the integration's own data is replaced. Core settings remain authoritative. */
export function applyAddonLaunchSettings(game:GameInfo,id:string,group:AddonCategory|null,data:Record<string,unknown>,overview:AddonOverview):AdvancedSettings {
 if(typeof data.enabled!=="boolean"||JSON.stringify(data).length>32*1024)throw new Error("Invalid launch integration configuration");
 let next=normalizeAdvanced(game.advanced);
 next.addon_launch={...next.addon_launch};
 if(data.enabled&&group==="frame-generation"){
  next.wrappers=next.wrappers?.map(wrapper=>isFrameGenerationWrapper(wrapper)?{...wrapper,enabled:false}:wrapper);
  next=releaseAddonLaunchGroup(next,group,overview);
 }
 delete next[id];
 next.addon_launch={...next.addon_launch,[id]:{enabled:data.enabled,group,data}};
 return next;
}

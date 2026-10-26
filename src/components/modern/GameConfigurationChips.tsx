import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {useI18n} from "../../i18n/I18nContext";
import {isFrameGenerationWrapper,type AdvancedSettings} from "../../services/advancedSettings";
import {gameConfigurationBadges,type MakoSummary} from "../../services/gameConfigurationBadges";
import {loadMangoHudProfile,type MangoHudSettings} from "../../services/mangohud";
import {experimentalFrameGeneration} from "../../services/releaseFeatures";

export function GameConfigurationChips({gameKey,advanced}:{gameKey:string;advanced?:AdvancedSettings}){
 const pt=useI18n().language==="pt";
 const [profile,setProfile]=useState<MangoHudSettings>();
 const [revision,setRevision]=useState(0);
 const [mako,setMako]=useState<{key:string;settings:MakoSummary}>();
 const wrapper=advanced?.schema_version===2?advanced.wrappers?.find(w=>w.enabled&&isFrameGenerationWrapper(w)):undefined;
 const requestKey=JSON.stringify([gameKey,wrapper?.program]);
 useEffect(()=>{let active=true;
  void loadMangoHudProfile().then(p=>{if(active)setProfile(p.settings)}).catch(()=>{});
  const changed=(event:Event)=>setProfile((event as CustomEvent<MangoHudSettings>).detail);
  window.addEventListener("mangohudProfileChanged",changed);
  return()=>{active=false;window.removeEventListener("mangohudProfileChanged",changed);};
 },[]);
 useEffect(()=>{const changed=()=>setRevision(v=>v+1);for(const name of ["peligamesLibraryChanged","frameGenerationProfileChanged","frameGenerationRuntimeChanged"])window.addEventListener(name,changed);return()=>{for(const name of ["peligamesLibraryChanged","frameGenerationProfileChanged","frameGenerationRuntimeChanged"])window.removeEventListener(name,changed);};},[]);
 useEffect(()=>{let active=true;setMako(undefined);
  if(experimentalFrameGeneration&&wrapper)void invoke<{settings:MakoSummary}>("frame_generation_state",{gameKey,backend:"saved",activeProfile:wrapper.program}).then(result=>{if(active)setMako({key:requestKey,settings:result.settings})}).catch(()=>{});
  return()=>{active=false};
 },[requestKey,revision]);
 const badges=gameConfigurationBadges(advanced,profile,wrapper&&mako?.key===requestKey?mako.settings:undefined,pt);
 return <>{badges.map(badge=><span key={badge.id} className="modern-meta-chip configuration" title={`${badge.description} ${pt?"Configuração salva; não confirma funcionamento em tempo real.":"Saved configuration; not a live runtime confirmation."}`} data-configuration={badge.id}>{badge.label}</span>)}</>;
}

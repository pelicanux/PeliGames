import {useAddonStore} from "../../modules/addons/store";
import {activeFrameGenerationAddons} from "../../modules/addons/launchSettings";
import {invoke} from "@tauri-apps/api/core";
import {useEffect,useState} from "react";
import {useI18n} from "../../i18n/I18nContext";
import {type AdvancedSettings} from "../../services/advancedSettings";
import {gameConfigurationBadges} from "../../services/gameConfigurationBadges";
import {loadMangoHudProfile,type MangoHudSettings} from "../../services/mangohud";

export function GameConfigurationChips({gameKey,advanced}:{gameKey:string;advanced?:AdvancedSettings}){
  const {t: tx, language} = useI18n();

 const [profile,setProfile]=useState<MangoHudSettings>();
 useEffect(()=>{let active=true;
  void loadMangoHudProfile().then(p=>{if(active)setProfile(p.settings)}).catch(()=>{});
  const changed=(event:Event)=>setProfile((event as CustomEvent<MangoHudSettings>).detail);
  window.addEventListener("mangohudProfileChanged",changed);
  return()=>{active=false;window.removeEventListener("mangohudProfileChanged",changed);};
 },[]);
 const signature=JSON.stringify([gameKey,advanced?.wrappers,advanced?.addon_launch]);
 const [legacy,setLegacy]=useState<{signature:string;selections:AdvancedSettings["addon_launch"]}>();
 useEffect(()=>{let active=true;if(advanced?.wrappers?.length)void invoke<AdvancedSettings["addon_launch"]>("addon_launch_settings",{advanced}).then(selections=>{if(active)setLegacy({signature,selections})}).catch(()=>{});return()=>{active=false}},[signature]);
 const effective=legacy?.signature===signature?{...advanced,addon_launch:legacy.selections} as AdvancedSettings:advanced;
 const overview=useAddonStore().overview;
 const external=activeFrameGenerationAddons(effective,overview);
 const badges=gameConfigurationBadges(advanced,profile,language);
 return <>{external.map(item=><span key={`addon:${item.id}`} className="modern-meta-chip configuration" title={tx("ui-game-configuration-chips","saved_configuration_not_a_live_runtime_confirmation")}>{item.name}</span>)}{badges.map(badge=><span key={badge.id} className="modern-meta-chip configuration" title={`${badge.description} ${tx("ui-game-configuration-chips", "saved_configuration_not_a_live_runtime_confirmation")}`} data-configuration={badge.id}>{badge.label}</span>)}</>;
}

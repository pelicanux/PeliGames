import {useEffect,useState} from "react";
import {experimentalFrameGeneration} from "../../services/releaseFeatures";
import {invoke} from "@tauri-apps/api/core";
import {MangoHudShortcutRecorder} from "../../components/preferences/MangoHudShortcutRecorder";
import {loadMangoHudProfile} from "../../services/mangohud";
import {shortcutConflict} from "../../components/preferences/shortcutConflicts";
import {useI18n} from "../../i18n/I18nContext";
import "./frameOverlay.css";
interface Preferences {shortcut:string}
interface Status {preferences:Preferences}
export function FrameOverlaySettings({compact=false,disabled=false}:{compact?:boolean;disabled?:boolean}){
  const {t: tx} = useI18n();

 if(!experimentalFrameGeneration)return null;

 const [state,setState]=useState<Status>();
 const [error,setError]=useState("");
 const [busy,setBusy]=useState(false);
 useEffect(()=>{let current=true;void invoke<Status>("frame_overlay_preferences").then(s=>{if(current)setState(s)}).catch(e=>{if(current)setError(String(e))});return()=>{current=false}},[]);
 const save=async(shortcut:string)=>{
  if(busy||disabled)return;
  setBusy(true);setError("");
  try{
   const profile=await loadMangoHudProfile();
   const conflict=shortcutConflict("frame_overlay",shortcut,profile.settings);
   if(conflict)throw new Error(tx("ui-frame-overlay-settings", "shortcut_already_used", {"conflict": conflict}));
   const preferences={shortcut};
   await invoke("configure_frame_overlay",{preferences});
   setState({preferences});
  }catch(e){setError(String(e))}finally{setBusy(false)}
 };
 return <div className={`fg-overlay-settings fg-overlay-shortcut-row${compact?"":" fg-overlay-global-settings"}`}>
  <div><strong>{tx("ui-frame-overlay-settings", "overlay_shortcut")}</strong><small>{tx("ui-frame-overlay-settings", "show_or_hide_the_controls_inside_the_game")}</small></div>
  {state&&<fieldset disabled={busy||disabled} className="mm-shortcut-actions fg-overlay-shortcut-actions"><MangoHudShortcutRecorder label={tx("ui-frame-overlay-settings", "show_or_hide_overlay")} value={state.preferences.shortcut} onChange={shortcut=>void save(shortcut)}/><button type="button" className="mm-ghost" aria-label={tx("ui-frame-overlay-settings", "restore_default_shortcut_f8")} title={tx("ui-frame-overlay-settings", "default_f8")} disabled={busy||disabled||state.preferences.shortcut==="F8"} onClick={()=>void save("F8")}>{tx("ui-frame-overlay-settings", "default")}</button></fieldset>}
  {!compact&&<small className="fg-overlay-shortcut-feedback">{tx("ui-frame-overlay-settings", "enable_the_overlay_in_each_game_s_details_shortcut_changes_apply_when_restarting_the_gam")}</small>}
  {error&&<small className="fg-overlay-shortcut-feedback" role="alert">{error}</small>}
 </div>
}

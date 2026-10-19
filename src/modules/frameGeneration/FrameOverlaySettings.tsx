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
 if(!experimentalFrameGeneration)return null;
 const pt=useI18n().language==="pt";
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
   if(conflict)throw new Error(pt?`Atalho já usado: ${conflict}.`:`Shortcut already used: ${conflict}.`);
   const preferences={shortcut};
   await invoke("configure_frame_overlay",{preferences});
   setState({preferences});
  }catch(e){setError(String(e))}finally{setBusy(false)}
 };
 return <div className={`fg-overlay-settings fg-overlay-shortcut-row${compact?"":" fg-overlay-global-settings"}`}>
  <div><strong>{pt?"Atalho do overlay":"Overlay shortcut"}</strong><small>{pt?"Mostra ou oculta os controles dentro do jogo.":"Show or hide the controls inside the game."}</small></div>
  {state&&<fieldset disabled={busy||disabled} className="mm-shortcut-actions fg-overlay-shortcut-actions"><MangoHudShortcutRecorder label={pt?"Mostrar ou ocultar overlay":"Show or hide overlay"} value={state.preferences.shortcut} onChange={shortcut=>void save(shortcut)}/><button type="button" className="mm-ghost" aria-label={pt?"Restaurar atalho padrão: F8":"Restore default shortcut: F8"} title={pt?"Padrão: F8":"Default: F8"} disabled={busy||disabled||state.preferences.shortcut==="F8"} onClick={()=>void save("F8")}>{pt?"Padrão":"Default"}</button></fieldset>}
  {!compact&&<small className="fg-overlay-shortcut-feedback">{pt?"Ative o overlay nos detalhes de cada jogo. Mudanças de tecla valem ao reabrir o jogo; não há registro de atalho global no desktop.":"Enable the overlay in each game's details. Shortcut changes apply when restarting the game; no desktop-wide shortcut is registered."}</small>}
  {error&&<small className="fg-overlay-shortcut-feedback" role="alert">{error}</small>}
 </div>
}

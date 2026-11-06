import type {Catalog} from "../i18n/engine";
import {useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {GameInfo} from "./GameGrid";
import {NexusPopup} from "../modules/nexus/NexusPopup";
import {MenuIcon} from "./MenuIcon";
import {useI18n} from "../i18n/I18nContext";
import "./GameCopyDialog.css";
import "./GameResetPanel.css";
export type GameResetSection="mods"|"dlss"|"frame_generation"|"performance"|"options"|"advanced"|"all";
const choices:{id:GameResetSection;labelKey:keyof Catalog["ui-game-reset-panel"];descriptionKey:keyof Catalog["ui-game-reset-panel"]}[]=[
 {id:"mods", labelKey: "reset_mods_0", descriptionKey: "reset_mods_0_description"},
 {id:"dlss", labelKey: "reset_dlss_0", descriptionKey: "reset_dlss_0_description"},
 {id:"frame_generation", labelKey: "reset_frame_generation_0", descriptionKey: "reset_frame_generation_0_description"},
 {id:"performance", labelKey: "reset_performance_0", descriptionKey: "reset_performance_0_description"},
 {id:"options", labelKey: "reset_options_0", descriptionKey: "reset_options_0_description"},
 {id:"advanced", labelKey: "reset_advanced_0", descriptionKey: "reset_advanced_0_description"},
 {id:"all", labelKey: "reset_all_0", descriptionKey: "reset_all_0_description"},
];
interface ResetResult{game:GameInfo;completed:string[];warnings:string[]}
export function GameResetPanel({game,nexusId,gpuArch,bitness,disabled,onBusy,onSaved}:{game:GameInfo;nexusId?:string;gpuArch:string;bitness:string;disabled?:boolean;onBusy:(busy:boolean)=>void;onSaved:(game:GameInfo)=>void}){
  const {t: tx} = useI18n();

 const [section,setSection]=useState<GameResetSection>("advanced"),[confirmation,setConfirmation]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState(""),[result,setResult]=useState<ResetResult|null>(null);
 const lock=useRef(false),choice=choices.find(c=>c.id===section)!;
 const reset=async()=>{
  if(lock.current||disabled)return;lock.current=true;setBusy(true);onBusy(true);setError("");setResult(null);
  try{
   if("__PELI_UI_PREVIEW__" in window){setResult({game,completed:[tx("ui-game-reset-panel", "preview_no_settings_or_files_were_changed")],warnings:[]});setConfirmation(false);return;}
   const value=await invoke<ResetResult>("reset_game_details",{request:{path:game.path,external_id:game.external_id||null,nexus_id:nexusId||null,section,gpu_arch:gpuArch,bitness}});
   setResult(value);onSaved({...game,...value.game,cover_url:game.cover_url,automatic_cover_url:game.automatic_cover_url,library_view:game.library_view});setConfirmation(false);
   window.dispatchEvent(new Event("peligamesLibraryChanged"));window.dispatchEvent(new Event("frameGenerationRuntimeChanged"));
  }catch(e){setError(String(e));}finally{lock.current=false;setBusy(false);onBusy(false);}
 };
 return <section className="game-reset-panel modern-performance-card">
  <div className="game-reset-header"><h3>{tx("ui-game-reset-panel", "reset")}</h3><div className="game-reset-pills" role="group" aria-label={tx("ui-game-reset-panel", "reset_scope")}>{choices.map(c=><button type="button" key={c.id} aria-pressed={section===c.id} disabled={disabled||busy} onClick={()=>{setSection(c.id);setError("");setResult(null);}}>{tx("ui-game-reset-panel", c.labelKey)}</button>)}</div></div>
  <p>{tx("ui-game-reset-panel", choice.descriptionKey)}</p>
  <div className="game-reset-action"><small>{tx("ui-game-reset-panel", "backups_saves_proton_wine_components_benchmarks_and_mangohud_appearance_are_preserved")}</small><button type="button" className="modern-mini-btn danger" disabled={disabled||busy} onClick={()=>{setError("");setResult(null);setConfirmation(true);}}><MenuIcon name="repair"/>{tx("ui-game-reset-panel", "reset_selection")}</button></div>
  {result&&<div role="status" className="game-reset-result">{result.completed.map(text=><p key={text}>{text}</p>)}{result.warnings.map(text=><p key={text}>{text}</p>)}</div>}
  {error&&!confirmation&&<p role="alert">{error}</p>}
  {confirmation&&<NexusPopup className="game-copy-dialog game-reset-confirm" title={`${tx("ui-game-reset-panel", "reset")} · ${tx("ui-game-reset-panel", choice.labelKey)}`} titleIcon={<MenuIcon name="repair"/>} onClose={()=>{if(!busy)setConfirmation(false);}} footer={<><button type="button" disabled={busy} onClick={()=>setConfirmation(false)}>{tx("ui-game-reset-panel", "cancel")}</button><button type="button" className="danger" disabled={busy} onClick={()=>void reset()}>{busy?(tx("ui-game-reset-panel", "resetting")):(tx("ui-game-reset-panel", "confirm_reset"))}</button></>}>
   <div className="game-reset-confirm-copy"><strong>{game.name}</strong><p>{section === "all" ? tx("ui-game-reset-panel", "confirm_all") : tx("ui-game-reset-panel", "confirm_selection")}</p><p>{tx("ui-game-reset-panel", choice.descriptionKey)}</p>{section==="all"&&<p>{tx("ui-game-reset-panel", "removes_managed_mods_and_packages_uninstalls_dlssnr_amd_disables_mako_overlay_and_clears")}</p>}<small>{tx("ui-game-reset-panel", "backups_are_preserved_along_with_wine_components_proton_benchmarks_and_mangohud_appearan")}</small>{error&&<p role="alert">{error}</p>}</div>
  </NexusPopup>}
 </section>;
}

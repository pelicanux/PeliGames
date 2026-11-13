import {useState} from "react";
import {useI18n} from "../i18n/I18nContext";
import type {MessageKey} from "../i18n/engine";
import {MenuIcon} from "./MenuIcon";

export interface ProfileExport {id:string;executable:string;arguments:string;line:string}
export type PeliRunTarget="Steam"|"Heroic"|"Lutris"|"Outro";

export function PeliRunInstructions({profile,initialTarget,copied,onCopy,onRefresh,disabled}:{profile:ProfileExport;initialTarget:PeliRunTarget;copied:string;onCopy:(key:string,value:string)=>void;onRefresh:()=>void;disabled:boolean}){
 const {t}=useI18n();const [target,setTarget]=useState(initialTarget);
 const tx=(key:MessageKey<"ui-peli-run-profile">)=>t("ui-peli-run-profile",key);
 const field=(label:string,key:string,value:string)=><label className="pelirun-export-field"><span>{label}</span><div><input aria-label={label} readOnly value={value} onFocus={event=>event.target.select()}/><button type="button" className="modern-mini-btn" onClick={()=>onCopy(`${target}:${key}`,value)}><MenuIcon name="copy"/>{tx(copied===`${target}:${key}`?"copied":"copy")}</button></div></label>;
 return <div className="pelirun-instructions">
  <header className="pelirun-ready"><div><MenuIcon name="check"/><strong>{tx("command_ready")}</strong></div><button type="button" className="modern-mini-btn" title={tx("refresh_command")} aria-label={tx("refresh_command")} disabled={disabled} onClick={onRefresh}><MenuIcon name="repair"/></button></header>
  <nav className="pelirun-export-targets" aria-label={tx("target_launcher")}>{(["Steam","Heroic","Lutris","Outro"] as const).map(item=><button key={item} type="button" className="modern-mini-btn" aria-pressed={target===item} onClick={()=>setTarget(item)}>{item==="Outro"?tx("other_launcher"):item}</button>)}</nav>
  <p className="pelirun-launcher-route">{target==="Steam"?tx("steam_destination"):target==="Heroic"?tx("heroic_destination"):target==="Lutris"?tx("lutris_destination"):tx("use_the_launcher_s_wrapper_or_command_prefix_setting_it_must_pass_the_complete_game_comm")}</p>
  {target==="Heroic"?<>{field(tx("executable"),"binary",profile.executable)}{field(tx("wrapper_arguments"),"args",profile.arguments)}</>:field(tx("command"),"line",`${profile.line}${target==="Steam"?" %command%":""}`)}
  <p className="pelirun-export-note">{tx("setup_once")}</p>
  <details className="pelirun-export-help"><summary>{tx("compatibility_details")}</summary><p>{tx("set_up_once_then_save_changes_in_peligames_and_relaunch_the_game_this_does_not_automatic")}</p><p>{tx("flatpak_the_launcher_needs_access_to_pelirun_its_profile_and_the_game_s_components_no_pe")}</p></details>
 </div>;
}

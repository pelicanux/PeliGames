import {GameSaveStatusIndicator} from "../GameSaveStatus";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useBoundedDropdown } from "../../hooks/useBoundedDropdown";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nContext";
import { advancedError, normalizeAdvanced, type AdvancedSettings } from "../../services/advancedSettings";
import { runWineTool } from "../../services/wineTools";
import { WineDllPriorities } from "../WineDllPriorities";
import type { InstalledProton } from "../ProtonSelector";
import { MenuIcon } from "../MenuIcon";
import { notifyError, notifyInfo } from "../../services/notifications";
import { WinetricksModal } from "../WinetricksModal";

export function ModernGameOptions({name="",value, proton, protonLabel, steamProtonMode, onProton, onAdvanced, launchOptions, disabled, toolsDisabled = disabled, native, maintenancePath, onUninstall, onChangeExecutable, automaticDlls, dllError, onResetDlls, children, protonDisabled, protonDisabledHint, protonDisabledLabel, configuration, prefixPrograms}: {
  name?: string; steamProtonMode?: "profile" | "steam"; protonLabel?: string; configuration?: ReactNode; prefixPrograms?: ReactNode; protonDisabled?: boolean; protonDisabledHint?: string; protonDisabledLabel?: string; value?: AdvancedSettings; proton: string; onProton: (path: string)=>Promise<boolean>; onAdvanced: (settings:AdvancedSettings)=>Promise<boolean>;
  launchOptions: ReactNode; disabled?: boolean; toolsDisabled?: boolean; native?: boolean; maintenancePath: string; onUninstall?: ()=>void; onChangeExecutable?: ()=>void;
  automaticDlls?: string; dllError?: string; onResetDlls?: ()=>void; children?: ReactNode;
}) {

  const {t} = useI18n();
  const [runners,setRunners]=useState<InstalledProton[]>([]), [runnerError,setRunnerError]=useState("");
  const [other,setOther]=useState(false), [runnerSearch,setRunnerSearch]=useState("");
  const runnerAnchor=useRef<HTMLDivElement>(null), runnerMenu=useRef<HTMLDivElement>(null);
  const runnerMenuId=useId();
  const runnerPosition=useBoundedDropdown(other,runnerAnchor,runnerMenu,setOther,320,"right");
  const filteredRunners=runners.filter(r=>r.name.toLocaleLowerCase().includes(runnerSearch.trim().toLocaleLowerCase()));
  const closeRunners=()=>{setOther(false);runnerAnchor.current?.querySelector<HTMLButtonElement>("[data-dropdown-trigger]")?.focus();};
  useEffect(()=>{if(disabled||protonDisabled||native)setOther(false);},[disabled,protonDisabled,native]);
  const [winetricks,setWinetricks]=useState(false);
  const [busy,setBusy]=useState(false), [activeTool,setActiveTool]=useState<string | null>(null);
  const lock=useRef(false), alive=useRef(true);
  useEffect(()=>{alive.current=true;if(native)return()=>{alive.current=false;};const scan=()=>{void invoke<InstalledProton[]>("scan_installed_protons").then(data=>{if(alive.current){setRunners(data);setRunnerError("");}}).catch(e=>{if(alive.current)setRunnerError(String(e));});};scan();window.addEventListener("protonRunnersChanged",scan);return()=>{alive.current=false;window.removeEventListener("protonRunnersChanged",scan);};},[native]);
  const run=async(action:()=>Promise<unknown>,message?:string,source=t("ui-modern-game-options", "game_options"),tool?:string)=>{
    if((tool ? toolsDisabled : disabled)||lock.current)return;
    lock.current=true;setBusy(true);setActiveTool(tool??null);
    try{await action();if(message)notifyInfo(source,message);}
    catch(e){notifyError(source,e);}
    finally{lock.current=false;if(alive.current){setBusy(false);setActiveTool(null);}}
  };
  const choose=(path:string)=>{if(native||protonDisabled)return;void run(async()=>{if(await onProton(path))closeRunners();else throw new Error(t("ui-modern-game-options", "could_not_save_proton"));});};
  const active=runners.find(r=>r.path===proton);
  const shortcuts=Array.from(new Map([...(proton?[{path:proton,name:active?.name||protonLabel||proton.split('/').pop()||proton}]:[]),...runners.filter(r=>r.path!==proton)].map(r=>[r.path,r])).values()).slice(0,2);
  const blocked=disabled||busy;
  return <div className="modern-game-options">
    <section className="modern-performance-card modern-execution-options"><div className="modern-panel-heading"><h3>{t("ui-modern-game-options", "execution")}</h3><GameSaveStatusIndicator/></div>
      {!native && !proton && protonLabel && <p className="modern-performance-hint">{t("ui-modern-game-options", "selected_in_steam")}{protonLabel}</p>}
      {native&&<div className="modern-option-row"><span>{t("ui-modern-game-options", "proton_version")}<small className="modern-performance-hint">{t("ui-modern-game-options", "native_linux_game_proton_is_not_used")}</small></span><button type="button" className="modern-mini-btn" disabled>{t("ui-modern-game-options", "native_linux")}</button></div>}
      {!native&&protonDisabled&&<div className="modern-option-row"><span>{t("ui-modern-game-options", "proton_version")}{protonDisabledHint&&<small className="modern-performance-hint">{protonDisabledHint}</small>}</span><button type="button" className="modern-mini-btn" disabled>{protonDisabledLabel||(t("ui-modern-game-options", "managed_externally"))}</button></div>}
      {!native&&!protonDisabled&&<><div className="modern-option-row"><span>{t("ui-modern-game-options", "proton_version")}</span><div ref={runnerAnchor} className="modern-dlss-segments modern-proton-segments" role="group" aria-label={t("ui-modern-game-options", "proton_version")}>{shortcuts.map(r=><button key={r.path} type="button" title={r.name} aria-pressed={proton===r.path} disabled={blocked} onClick={()=>choose(r.path)}>{r.name}</button>)}<button type="button" data-dropdown-trigger disabled={blocked} aria-haspopup="dialog" aria-controls={other?runnerMenuId:undefined} aria-expanded={other} onClick={()=>{setRunnerSearch("");setOther(v=>!v);}}>{t("ui-modern-game-options", "other")}</button></div></div>
      {other&&createPortal(<div ref={runnerMenu} id={runnerMenuId} className="modern-proton-picker" role="dialog" aria-label={t("ui-modern-game-options", "other_proton_versions")} style={runnerPosition} onKeyDown={event=>{
        if(event.key==="Escape"){event.stopPropagation();closeRunners();}
        if(event.key==="Tab")setOther(false);
        if(event.key==="ArrowDown"||event.key==="ArrowUp"){
          event.preventDefault();
          const options=Array.from(runnerMenu.current?.querySelectorAll<HTMLButtonElement>("[role=option]")??[]);
          const index=options.indexOf(document.activeElement as HTMLButtonElement);
          options[(index+(event.key==="ArrowDown"?1:-1)+options.length)%options.length]?.focus();
        }
      }}>
        <label className="modern-proton-search"><MenuIcon name="search"/><input autoFocus type="search" value={runnerSearch} onChange={event=>setRunnerSearch(event.target.value)} placeholder={t("ui-modern-game-options", "search_proton_versions")} aria-label={t("ui-modern-game-options", "search_proton_versions")}/></label>
        <div className="modern-proton-picker-list" role="listbox" aria-label={t("ui-modern-game-options", "installed_versions")} aria-busy={busy}>
          {filteredRunners.map(r=><button type="button" role="option" key={r.path} disabled={blocked||protonDisabled} aria-selected={r.path===proton} onClick={()=>choose(r.path)}><span>{r.name}</span>{r.path===proton&&<MenuIcon name="check"/>}</button>)}
          {!filteredRunners.length&&<p role="status">{runnerError||((runners.length ? t("ui-modern-game-options", "no_matching_versions") : t("ui-modern-game-options", "no_proton_versions_installed")))}</p>}
        </div>
        <button type="button" className="modern-proton-manage" onClick={()=>{closeRunners();window.dispatchEvent(new Event("openProtonPreferences"));}}>{t("ui-modern-game-options", "manage_versions")}</button>
      </div>,document.body)}</>}

      {!native&&steamProtonMode&&<div className="modern-option-row"><span>{t("ui-modern-game-options", "proton_selection")}<small className="modern-performance-hint">{steamProtonMode==="profile"?(t("ui-modern-game-options", "saved_in_the_peligames_profile_used_on_the_next_game_launch_without_restarting_steam")):(t("ui-modern-game-options", "following_steam_s_selection_choose_a_version_above_to_pin_it_to_this_profile"))}</small></span>{steamProtonMode==="profile"&&<button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>choose("")}>{t("ui-modern-game-options", "use_steam_s_proton")}</button>}</div>}

      {!native&&!maintenancePath.startsWith("external:")&&<div className="modern-option-row"><span>{t("ui-modern-game-options", "steam_client_communication")}<small className="modern-performance-hint">{t("ui-modern-game-options", "requires_steam_to_be_running_and_a_compatible_proton_does_not_add_the_game_to_steam")}</small></span><button type="button" role="switch" aria-label={t("ui-modern-game-options", "steam_client_communication")} aria-checked={Boolean(value?.options.steam_client)} className={`advanced-toggle ${value?.options.steam_client?"enabled":""}`} disabled={blocked} onClick={()=>void run(async()=>{const current=normalizeAdvanced(value);if(!await onAdvanced({...current,options:{...current.options,steam_client:!current.options.steam_client}}))throw new Error(t("ui-modern-game-options", "could_not_save_steam_client_communication"));})}><span/></button></div>}
      {launchOptions}
      {!native&&<WineDllPriorities compact settings={normalizeAdvanced(value)} disabled={blocked} automatic={automaticDlls} detectionError={dllError} onReset={onResetDlls} onChange={next=>{const problem=advancedError(next);if(problem){notifyError(t("ui-modern-game-options", "wine_dll_priorities"),problem);return false;}void run(async()=>{if(!await onAdvanced(next))throw new Error(t("ui-modern-game-options", "could_not_save_dll_priorities"));});return true;}}/>}
      {configuration}
      {children&&<div className="modern-options-section">{children}</div>}
    </section>
    <section className="modern-performance-card modern-maintenance-options"><h3>{t("ui-modern-game-options", "maintenance")}</h3>
      {!native&&<section className="modern-wine-section" aria-label={t("ui-modern-game-options", "wine_tools")}><h4>{t("ui-modern-game-options", "wine_tools")}</h4>
      <div className="modern-wine-tools">{([['winecfg','Winecfg','settings'],['winetricks-gui','Winetricks','cube'],['regedit','Regedit','document'],['explorer',t("ui-modern-game-options", "file_explorer"),'folder']] as const).map(([action,label,icon])=><button type="button" className="modern-mini-btn" key={action} disabled={toolsDisabled||busy||!proton} aria-busy={activeTool===action} title={activeTool===action?(t("ui-modern-game-options", "preparing_or_running_the_tool_please_wait")):undefined} onClick={()=>action==="winetricks-gui"?setWinetricks(true):void run(()=>runWineTool(maintenancePath,action),undefined,label,action)}>{activeTool===action?<span className="modern-wine-tool-spinner" aria-hidden="true"/>:<MenuIcon name={icon}/>}<span>{label}</span></button>)}{!proton&&<p>{t("ui-modern-game-options", "select_a_proton_version_to_use_these_tools")}</p>}</div></section>}
      {onChangeExecutable&&<div className="modern-option-row"><span>{t("gameInfo","changeExecutable")}</span><button type="button" className="modern-mini-btn" disabled={blocked} onClick={onChangeExecutable}>{t("ui-modern-game-options", "choose")}</button></div>}
      <div className="modern-option-row"><span>{t("ui-modern-game-options", "desktop_shortcut")}</span><button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>void run(()=>invoke<string>("create_peligames_shortcut",{path:maintenancePath}),t("ui-modern-game-options", "desktop_shortcut_created"))}>{t("ui-modern-game-options", "create")}</button></div>
      <div className="modern-option-row modern-uninstall-row"><span>{t("ui-modern-game-options", "uninstall_game")}</span><button type="button" className="modern-mini-btn danger" disabled={blocked||!onUninstall} onClick={onUninstall}>{t("ui-modern-game-options", "uninstall")}</button></div>
      {!onUninstall&&<p className="modern-performance-hint">{t("ui-modern-game-options", "uninstallation_is_managed_by_the_source_launcher")}</p>}
      {prefixPrograms}
      {winetricks&&<WinetricksModal path={maintenancePath} name={name} proton={proton} onClose={()=>setWinetricks(false)}/>}
    </section>
  </div>;
}

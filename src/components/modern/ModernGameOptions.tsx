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

export function ModernGameOptions({name="",value, proton, protonLabel, onProton, onAdvanced, launchOptions, disabled, native, maintenancePath, onUninstall, onChangeExecutable, automaticDlls, dllError, onResetDlls, children, protonDisabled, protonDisabledHint, protonDisabledLabel, configuration, prefixPrograms}: {
  name?: string; protonLabel?: string; configuration?: ReactNode; prefixPrograms?: ReactNode; protonDisabled?: boolean; protonDisabledHint?: string; protonDisabledLabel?: string; value?: AdvancedSettings; proton: string; onProton: (path: string)=>Promise<boolean>; onAdvanced: (settings:AdvancedSettings)=>Promise<boolean>;
  launchOptions: ReactNode; disabled?: boolean; native?: boolean; maintenancePath: string; onUninstall?: ()=>void; onChangeExecutable?: ()=>void;
  automaticDlls?: string; dllError?: string; onResetDlls?: ()=>void; children?: ReactNode;
}) {
  const {language,t} = useI18n(), pt=language==="pt";
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
  const run=async(action:()=>Promise<unknown>,message?:string,source=pt?"Opções do jogo":"Game options",tool?:string)=>{
    if(disabled||lock.current)return;
    lock.current=true;setBusy(true);setActiveTool(tool??null);
    try{await action();if(message)notifyInfo(source,message);}
    catch(e){notifyError(source,e);}
    finally{lock.current=false;if(alive.current){setBusy(false);setActiveTool(null);}}
  };
  const choose=(path:string)=>{if(native||protonDisabled)return;void run(async()=>{if(await onProton(path))closeRunners();else throw new Error(pt?"Não foi possível salvar o Proton.":"Could not save Proton.");});};
  const active=runners.find(r=>r.path===proton);
  const shortcuts=Array.from(new Map([...(proton?[{path:proton,name:protonLabel||active?.name||proton.split('/').pop()||proton}]:[]),...runners.filter(r=>r.path!==proton)].map(r=>[r.path,r])).values()).slice(0,2);
  const blocked=disabled||busy;
  return <div className="modern-game-options">
    <section className="modern-performance-card modern-execution-options"><h3>{pt?"Execução":"Execution"}</h3>
      {!native && !proton && protonLabel && <p className="modern-performance-hint">{pt ? "Selecionado na Steam: " : "Selected in Steam: "}{protonLabel}</p>}
      {native&&<div className="modern-option-row"><span>{pt?"Versão do Proton":"Proton version"}<small className="modern-performance-hint">{pt?"Jogo nativo de Linux; não utiliza Proton.":"Native Linux game; Proton is not used."}</small></span><button type="button" className="modern-mini-btn" disabled>{pt?"Linux nativo":"Native Linux"}</button></div>}
      {!native&&protonDisabled&&<div className="modern-option-row"><span>{pt?"Versão do Proton":"Proton version"}{protonDisabledHint&&<small className="modern-performance-hint">{protonDisabledHint}</small>}</span><button type="button" className="modern-mini-btn" disabled>{protonDisabledLabel||(pt?"Gerenciado externamente":"Managed externally")}</button></div>}
      {!native&&!protonDisabled&&<><div className="modern-option-row"><span>{pt?"Versão do Proton":"Proton version"}</span><div ref={runnerAnchor} className="modern-dlss-segments modern-proton-segments" role="group" aria-label={pt?"Versão do Proton":"Proton version"}>{shortcuts.map(r=><button key={r.path} type="button" title={r.name} aria-pressed={proton===r.path} disabled={blocked} onClick={()=>choose(r.path)}>{r.name}</button>)}<button type="button" data-dropdown-trigger disabled={blocked} aria-haspopup="dialog" aria-controls={other?runnerMenuId:undefined} aria-expanded={other} onClick={()=>{setRunnerSearch("");setOther(v=>!v);}}>{pt?"Outra…":"Other…"}</button></div></div>
      {other&&createPortal(<div ref={runnerMenu} id={runnerMenuId} className="modern-proton-picker" role="dialog" aria-label={pt?"Outras versões do Proton":"Other Proton versions"} style={runnerPosition} onKeyDown={event=>{
        if(event.key==="Escape"){event.stopPropagation();closeRunners();}
        if(event.key==="Tab")setOther(false);
        if(event.key==="ArrowDown"||event.key==="ArrowUp"){
          event.preventDefault();
          const options=Array.from(runnerMenu.current?.querySelectorAll<HTMLButtonElement>("[role=option]")??[]);
          const index=options.indexOf(document.activeElement as HTMLButtonElement);
          options[(index+(event.key==="ArrowDown"?1:-1)+options.length)%options.length]?.focus();
        }
      }}>
        <label className="modern-proton-search"><MenuIcon name="search"/><input autoFocus type="search" value={runnerSearch} onChange={event=>setRunnerSearch(event.target.value)} placeholder={pt?"Buscar versão do Proton":"Search Proton versions"} aria-label={pt?"Buscar versão do Proton":"Search Proton versions"}/></label>
        <div className="modern-proton-picker-list" role="listbox" aria-label={pt?"Versões instaladas":"Installed versions"} aria-busy={busy}>
          {filteredRunners.map(r=><button type="button" role="option" key={r.path} disabled={blocked||protonDisabled} aria-selected={r.path===proton} onClick={()=>choose(r.path)}><span>{r.name}</span>{r.path===proton&&<MenuIcon name="check"/>}</button>)}
          {!filteredRunners.length&&<p role="status">{runnerError||(pt?(runners.length?"Nenhuma versão encontrada.":"Nenhuma versão do Proton instalada."):(runners.length?"No matching versions.":"No Proton versions installed."))}</p>}
        </div>
        <button type="button" className="modern-proton-manage" onClick={()=>{closeRunners();window.dispatchEvent(new Event("openProtonPreferences"));}}>{pt?"Gerenciar versões":"Manage versions"}</button>
      </div>,document.body)}</>}

      {!native&&!maintenancePath.startsWith("external:")&&<div className="modern-option-row"><span>{pt?"Comunicação com a Steam":"Steam client communication"}<small className="modern-performance-hint">{pt?"Requer a Steam aberta e um Proton compatível. Não adiciona o jogo à Steam.":"Requires Steam to be running and a compatible Proton. Does not add the game to Steam."}</small></span><button type="button" role="switch" aria-label={pt?"Comunicação com a Steam":"Steam client communication"} aria-checked={Boolean(value?.options.steam_client)} className={`advanced-toggle ${value?.options.steam_client?"enabled":""}`} disabled={blocked} onClick={()=>void run(async()=>{const current=normalizeAdvanced(value);if(!await onAdvanced({...current,options:{...current.options,steam_client:!current.options.steam_client}}))throw new Error(pt?"Não foi possível salvar a comunicação com a Steam.":"Could not save Steam client communication.");})}><span/></button></div>}
      {launchOptions}
      {!native&&<WineDllPriorities compact settings={normalizeAdvanced(value)} disabled={blocked} automatic={automaticDlls} detectionError={dllError} onReset={onResetDlls} onChange={next=>{const problem=advancedError(next);if(problem){notifyError(pt?"Prioridade das DLLs do Wine":"Wine DLL priorities",problem);return false;}void run(async()=>{if(!await onAdvanced(next))throw new Error(pt?"Não foi possível salvar as DLLs.":"Could not save DLL priorities.");});return true;}}/>}
      {configuration}
      {children&&<div className="modern-options-section">{children}</div>}
    </section>
    <section className="modern-performance-card modern-maintenance-options"><h3>{pt?"Manutenção":"Maintenance"}</h3>
      {!native&&<section className="modern-wine-section" aria-label={pt?"Ferramentas do Wine":"Wine tools"}><h4>{pt?"Ferramentas do Wine":"Wine tools"}</h4>
      <div className="modern-wine-tools">{([['winecfg','Winecfg','settings'],['winetricks-gui','Winetricks','cube'],['regedit','Regedit','document'],['explorer',pt?'Explorador de arquivos':'File explorer','folder']] as const).map(([action,label,icon])=><button type="button" className="modern-mini-btn" key={action} disabled={blocked||!proton} aria-busy={activeTool===action} title={activeTool===action?(pt?"Preparando ou executando a ferramenta. Aguarde…":"Preparing or running the tool. Please wait…"):undefined} onClick={()=>action==="winetricks-gui"?setWinetricks(true):void run(()=>runWineTool(maintenancePath,action),undefined,label,action)}>{activeTool===action?<span className="modern-wine-tool-spinner" aria-hidden="true"/>:<MenuIcon name={icon}/>}<span>{label}</span></button>)}{!proton&&<p>{pt?"Selecione uma versão do Proton para usar as ferramentas.":"Select a Proton version to use these tools."}</p>}</div></section>}
      {onChangeExecutable&&<div className="modern-option-row"><span>{t("gameInfo","changeExecutable")}</span><button type="button" className="modern-mini-btn" disabled={blocked} onClick={onChangeExecutable}>{pt?"Escolher":"Choose"}</button></div>}
      <div className="modern-option-row"><span>{pt?"Atalho na área de trabalho":"Desktop shortcut"}</span><button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>void run(()=>invoke<string>("create_peligames_shortcut",{path:maintenancePath}),pt?"Atalho criado na área de trabalho.":"Desktop shortcut created.")}>{pt?"Criar":"Create"}</button></div>
      <div className="modern-option-row modern-uninstall-row"><span>{pt?"Desinstalar jogo":"Uninstall game"}</span><button type="button" className="modern-mini-btn danger" disabled={blocked||!onUninstall} onClick={onUninstall}>{pt?"Desinstalar":"Uninstall"}</button></div>
      {!onUninstall&&<p className="modern-performance-hint">{pt?"A desinstalação é gerenciada pelo launcher de origem.":"Uninstallation is managed by the source launcher."}</p>}
      {prefixPrograms}
      {winetricks&&<WinetricksModal path={maintenancePath} name={name} proton={proton} onClose={()=>setWinetricks(false)}/>}
    </section>
  </div>;
}

import {useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import type {GameInfo} from "./GameGrid";
import {NexusPopup} from "../modules/nexus/NexusPopup";
import {MenuIcon} from "./MenuIcon";
import {useI18n} from "../i18n/I18nContext";
import "./GameCopyDialog.css";
import "./GameResetPanel.css";
export type GameResetSection="mods"|"dlss"|"frame_generation"|"performance"|"options"|"advanced"|"all";
const choices:{id:GameResetSection;pt:string;en:string;description:string;descriptionEn:string}[]=[
 {id:"mods",pt:"Mods",en:"Mods",description:"Remove os mods gerenciados e os pacotes baixados. Restaura os arquivos originais registrados pelo gerenciador. Arquivos desconhecidos ou alterados por fora podem exigir intervenção manual.",descriptionEn:"Removes managed mods and downloaded packages, restoring tracked original files. Unknown or externally modified files may require manual intervention."},
 {id:"dlss",pt:"DLSSNR-AMD",en:"DLSSNR-AMD",description:"Executa a desinstalação do DLSSNR-AMD e remove suas opções de inicialização gerenciadas.",descriptionEn:"Uninstalls DLSSNR-AMD and removes its managed launch settings."},
 {id:"frame_generation",pt:"Geração de quadros",en:"Frame generation",description:"Desativa o MAKO neste jogo. Os componentes compartilhados continuam disponíveis para os outros jogos.",descriptionEn:"Disables MAKO for this game. Shared components remain available to other games."},
 {id:"performance",pt:"Desempenho",en:"Performance",description:"Desliga o overlay, o limite sem foco e o MangoHUD individual; remove os limites de FPS deste jogo, incluindo o limitador do Gamescope. O visual do MangoHUD e os benchmarks são preservados.",descriptionEn:"Disables this game's overlay, background cap, MangoHUD and FPS limits, including the Gamescope limiter. MangoHUD appearance and benchmarks are preserved."},
 {id:"options",pt:"Opções",en:"Options",description:"Limpa opções de inicialização, prioridades de DLLs e a ponte com o cliente Steam. Preserva a versão do Proton e os componentes instalados no prefixo.",descriptionEn:"Clears launch options, DLL priorities and the Steam client bridge. Preserves the selected Proton and installed prefix components."},
 {id:"advanced",pt:"Avançado",en:"Advanced",description:"Desativa os ajustes avançados, Gamescope e wrappers personalizados. Preserva as opções de inicialização, DLLs, monitoramento individual e a configuração do MAKO. Comandos escritos manualmente em Opções podem continuar ativando recursos.",descriptionEn:"Disables advanced tweaks, Gamescope and custom wrappers. Preserves launch options, DLL priorities, per-game monitoring and MAKO settings. Commands entered manually in Options may still enable features."},
 {id:"all",pt:"Tudo",en:"All",description:"Aplica todas as redefinições acima somente nesta instalação. Não reinstala nem apaga o jogo ou o prefixo.",descriptionEn:"Applies all resets above to this installation only. Does not reinstall or delete the game or prefix."},
];
interface ResetResult{game:GameInfo;completed:string[];warnings:string[]}
export function GameResetPanel({game,nexusId,gpuArch,bitness,disabled,onBusy,onSaved}:{game:GameInfo;nexusId?:string;gpuArch:string;bitness:string;disabled?:boolean;onBusy:(busy:boolean)=>void;onSaved:(game:GameInfo)=>void}){
 const pt=useI18n().language==="pt";
 const [section,setSection]=useState<GameResetSection>("advanced"),[confirmation,setConfirmation]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState(""),[result,setResult]=useState<ResetResult|null>(null);
 const lock=useRef(false),choice=choices.find(c=>c.id===section)!;
 const reset=async()=>{
  if(lock.current||disabled)return;lock.current=true;setBusy(true);onBusy(true);setError("");setResult(null);
  try{
   if("__PELI_UI_PREVIEW__" in window){setResult({game,completed:[pt?"Prévia: nenhuma configuração ou arquivo foi alterado.":"Preview: no settings or files were changed."],warnings:[]});setConfirmation(false);return;}
   const value=await invoke<ResetResult>("reset_game_details",{request:{path:game.path,external_id:game.external_id||null,nexus_id:nexusId||null,section,gpu_arch:gpuArch,bitness}});
   setResult(value);onSaved({...game,...value.game,cover_url:game.cover_url,automatic_cover_url:game.automatic_cover_url,library_view:game.library_view});setConfirmation(false);
   window.dispatchEvent(new Event("peligamesLibraryChanged"));window.dispatchEvent(new Event("frameGenerationRuntimeChanged"));
  }catch(e){setError(String(e));}finally{lock.current=false;setBusy(false);onBusy(false);}
 };
 return <section className="game-reset-panel modern-performance-card">
  <div className="game-reset-header"><h3>{pt?"Redefinir":"Reset"}</h3><div className="game-reset-pills" role="group" aria-label={pt?"O que redefinir":"Reset scope"}>{choices.map(c=><button type="button" key={c.id} aria-pressed={section===c.id} disabled={disabled||busy} onClick={()=>{setSection(c.id);setError("");setResult(null);}}>{pt?c.pt:c.en}</button>)}</div></div>
  <p>{pt?choice.description:choice.descriptionEn}</p>
  <div className="game-reset-action"><small>{pt?"Backups, saves, Proton, componentes Wine, benchmarks e visual do MangoHUD são preservados.":"Backups, saves, Proton, Wine components, benchmarks and MangoHUD appearance are preserved."}</small><button type="button" className="modern-mini-btn danger" disabled={disabled||busy} onClick={()=>{setError("");setResult(null);setConfirmation(true);}}><MenuIcon name="repair"/>{pt?"Redefinir seleção":"Reset selection"}</button></div>
  {result&&<div role="status" className="game-reset-result">{result.completed.map(text=><p key={text}>{text}</p>)}{result.warnings.map(text=><p key={text}>{text}</p>)}</div>}
  {error&&!confirmation&&<p role="alert">{error}</p>}
  {confirmation&&<NexusPopup className="game-copy-dialog game-reset-confirm" title={`${pt?"Redefinir":"Reset"} · ${pt?choice.pt:choice.en}`} titleIcon={<MenuIcon name="repair"/>} onClose={()=>{if(!busy)setConfirmation(false);}} footer={<><button type="button" disabled={busy} onClick={()=>setConfirmation(false)}>{pt?"Cancelar":"Cancel"}</button><button type="button" className="danger" disabled={busy} onClick={()=>void reset()}>{busy?(pt?"Redefinindo…":"Resetting…"):(pt?"Confirmar redefinição":"Confirm reset")}</button></>}>
   <div className="game-reset-confirm-copy"><strong>{game.name}</strong><p>{pt?`Tem certeza que deseja redefinir ${section==="all"?"todas essas opções":"esta seleção"}?`:`Reset ${section==="all"?"all these settings":"this selection"}?`}</p><p>{pt?choice.description:choice.descriptionEn}</p>{section==="all"&&<p>{pt?"Remove os mods gerenciados e seus pacotes, desinstala DLSSNR-AMD, desativa MAKO/overlay e limpa desempenho, opções e ajustes avançados. Arquivos desconhecidos podem permanecer.":"Removes managed mods and packages, uninstalls DLSSNR-AMD, disables MAKO/overlay and clears performance, launch and advanced settings. Unknown files may remain."}</p>}<small>{pt?"Os backups não serão apagados. Componentes do Wine, Proton, benchmarks e o visual do MangoHUD continuam como estão. Mods que alteram saves podem exigir uma cópia de save compatível após a remoção.":"Backups are preserved, along with Wine components, Proton, benchmarks and MangoHUD appearance. Mods that affect saves may require a compatible save after removal."}</small>{error&&<p role="alert">{error}</p>}</div>
  </NexusPopup>}
 </section>;
}

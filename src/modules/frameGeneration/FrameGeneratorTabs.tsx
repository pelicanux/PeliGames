import {useState,type ComponentProps} from "react";
import {FrameGenerationPanel} from "./FrameGenerationPanel";
import {AfmfPanel} from "./AfmfPanel";
import {normalizeAdvanced} from "../../services/advancedSettings";
import {useI18n} from "../../i18n/I18nContext";

type Generator="mako"|"afmf";
const GENERATORS:{id:Generator;label:string}[]=[{id:"mako",label:"MAKO"},{id:"afmf",label:"AFMF"}];

/** The game's frame generators side by side, as pill tabs; each keeps its own panel. */
export function FrameGeneratorTabs(props:ComponentProps<typeof FrameGenerationPanel>){
 const pt=useI18n().language==="pt";
 const [tab,setTab]=useState<Generator>(()=>normalizeAdvanced(props.game.advanced).afmf?.enabled?"afmf":"mako");
 const select=(next:Generator)=>{setTab(next);requestAnimationFrame(()=>document.getElementById(`fg-generator-tab-${next}`)?.focus())};
 return <div className="fg-generators">
  <div className="settings-tab-toolbar"><div className="preference-mod-tabs fg-generator-tabs" role="tablist" aria-label={pt?"Gerador de quadros":"Frame generator"} data-active={tab}
   onKeyDown={event=>{if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;event.preventDefault();select(event.key==="Home"?"mako":event.key==="End"?"afmf":tab==="mako"?"afmf":"mako")}}>
   {GENERATORS.map(g=><button key={g.id} type="button" id={`fg-generator-tab-${g.id}`} role="tab" aria-selected={tab===g.id} aria-controls={`fg-generator-panel-${g.id}`} tabIndex={tab===g.id?0:-1} onClick={()=>setTab(g.id)}>{g.label}</button>)}
  </div></div>
  <div id={`fg-generator-panel-${tab}`} role="tabpanel" aria-labelledby={`fg-generator-tab-${tab}`}>
   {tab==="mako"?<FrameGenerationPanel {...props}/>:<AfmfPanel game={props.game} disabled={props.disabled} running={props.running} onSaved={props.onSaved} onPersist={props.onPersist}/>}
  </div>
 </div>;
}

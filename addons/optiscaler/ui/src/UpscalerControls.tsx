import type {OptiScalerMessageKey} from "./messages";
import {ModOptionSwitch,SelectionMenu,SettingsCategories,useI18n} from "./environment";
import {useEffect,useState,useSyncExternalStore,type ReactNode} from "react";
import {createUpscalerChoice} from "./choice";
import {OptiScalerMenuKey} from "./OptiScalerMenuKey";
import {upscalerFields,upscalerValueLabel,upscalerDisplayValue,upscalerToggle,upscalerFieldAvailable,upscalerFieldValues} from "./settings";
import "./upscaler.css";

const settingCategories=["main","upscaling","framegeneration","latency"] as const;
type Category=typeof settingCategories[number]|"management"|"usage";
const icons={management:"download",main:"settings",upscaling:"graphics",framegeneration:"bolt",latency:"chip",usage:"info"} as const;
export function UpscalerControls({choice,installed,blocked,onBusy,framegenAvailable=true,framegenModern=true,management,compatibility,usage,configurationAvailable=true}:{choice:ReturnType<typeof createUpscalerChoice>;installed:boolean;blocked:boolean;onBusy?:(busy:boolean)=>void;framegenAvailable?:boolean;framegenModern?:boolean;management?:ReactNode;compatibility?:ReactNode;usage?:ReactNode;configurationAvailable?:boolean}){
 const {t}=useI18n();const tx=(key:OptiScalerMessageKey)=>t("upscaler",key);
 const categories:Category[]=[...(management?["management" as const]:[]),...settingCategories,...(usage?["usage" as const]:[])];
 const available=(group:Category)=>group==="management"||group==="usage"||configurationAvailable;
 const [category,setCategory]=useState<Category>(management&&(!installed||!configurationAvailable)?"management":"main");
 useEffect(()=>{if(management&&!configurationAvailable)setCategory("management")},[Boolean(management),configurationAvailable]);
 const state=useSyncExternalStore(choice.subscribe,choice.getSnapshot,choice.getSnapshot);
 useEffect(()=>{onBusy?.(state.busy);return()=>onBusy?.(false)},[state.busy,onBusy]);
 return <>
  <SettingsCategories label="OptiScaler" active={category} onChange={setCategory} categories={categories.map(group=>({
   id:group,label:tx(group),icon:icons[group],disabled:!available(group),disabledHint:tx("configuration_locked"),content:<>
    <header className="upscaler-category-heading"><h3>{tx(group)}</h3><p>{tx(group==="framegeneration"&&!framegenAvailable?"fg_unavailable":`${group}_hint`)}</p></header>
    {group==="management"?management:group==="usage"?usage:<>
    {[...new Set(upscalerFields.filter(field=>field.group===group).map(field=>field.section))].map(section=><fieldset className="upscaler-setting-section" key={section}>
     <legend>{tx(section)}</legend>
    {upscalerFields.filter(field=>field.group===group&&field.section===section&&upscalerFieldAvailable(field.key,state.values)&&(field.key!=="FrameGen.FGNvngxReplacement"||framegenModern)).map(field=>{const rawValue=state.values[field.key]??"auto";const value=upscalerDisplayValue(field.key,rawValue);
     if(field.key==="Menu.ShortcutKey")return <OptiScalerMenuKey key={field.key} value={rawValue} disabled={blocked||!configurationAvailable} onChange={value=>choice.change(field.key,value)}/>;
     const toggle=upscalerToggle(field.key,rawValue);
     const disabled=blocked||!configurationAvailable||(field.group==="framegeneration"&&!framegenAvailable);
     if(toggle)return <div className="upscaler-control-row upscaler-toggle" key={field.key}><div><span>{tx(field.label)}</span><small>{toggle.inherited?tx("auto"):tx(toggle.enabled?"on":"off")}</small></div><ModOptionSwitch appearance="rail" label={tx(field.label)} enabled={toggle.enabled} disabled={disabled} onToggle={()=>choice.change(field.key,toggle.next)}/></div>;
     let options: {value:string;label:string;disabled?:boolean}[]=upscalerFieldValues(field,state.values).map(value=>({value,label:upscalerValueLabel(field.key,value,tx("auto"),tx("on"),tx("off"))}));
     if(field.key==="FSR.Fsr4GpuSupport")options=field.values.map(value=>({value,label:tx(value==="auto"?"auto":value==="0"?"fsr4_no_override":value==="1"?"fsr4_fp8":value==="2"?"fsr4_int8_model":value==="3"?"fsr4_rdna2":"fsr4_lite")}));
     if(field.key==="FSR.Fsr4Model")options=field.values.map(value=>({value,label:tx(value==="auto"?"auto":value==="0"?"fsr4_fp8":value==="1"?"fsr4_int8_model":value==="2"?"fsr4_rdna2":"fsr4_lite")}));
     if(!framegenModern&&(field.key==="FrameGen.FGInput"||field.key==="FrameGen.FGOutput"))options=options.filter(o=>!["nvngxfg","dlssg","xefg"].includes(o.value)||o.value==="dlssg"&&field.key==="FrameGen.FGInput"||o.value==="xefg"&&field.key==="FrameGen.FGOutput").concat({value:"nukems",label:"Nukem · FSR 3 FG"});
     if(field.key==="fakenvapi.LatencyFlexMode")options=field.values.map(value=>({value,label:tx(value==="0"?"conservative":value==="1"?"aggressive":value==="2"?"reflex_ids":"auto")}));
     if(!options.some(o=>o.value===value))options.push({value,label:value==="auto"?tx("auto"):`${tx("custom")}: ${value}`,disabled:true});
     return <div className="upscaler-control-row upscaler-setting" key={field.key}><span>{tx(field.label)}</span><SelectionMenu label={tx(field.label)} value={value} options={options} appearance="compact" disabled={disabled} onChange={value=>choice.change(field.key,value)}/></div>;
    })}
    {section==="fsr_settings"&&<p className="upscaler-field-hint">{tx("fsr_library_hint")}{(state.values["FSR.Fsr4GpuSupport"]!==undefined||state.values["FSR.Fsr4ForceEnableInt8"]!==undefined)&&<> {tx("fsr4_compatibility_hint")}</>}</p>}
    </fieldset>)}
    {group==="main"&&compatibility}
    </>}
   </>
  }))}/>
  {((category!=="management"&&category!=="usage")||state.error||state.busy)&&<p role={state.error?"alert":"status"} className={state.error?"neural-startup-error":"modern-dlss-key-hint"}>{state.error?`${tx("save_error")} ${state.error}`:state.busy?tx("saving"):tx(installed?"saved":"draft")}</p>}
 </>;
}

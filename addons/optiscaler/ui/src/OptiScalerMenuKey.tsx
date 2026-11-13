import {useI18n} from "./environment";
import {useEffect,useRef,useState} from "react";
import {optiscalerMenuKey,optiscalerMenuKeyLabel} from "./menuShortcut";

export function OptiScalerMenuKey({value,disabled,onChange}:{value:string;disabled:boolean;onChange:(value:string)=>void}){
 const {t}=useI18n();
 const [recording,setRecording]=useState(false),[invalid,setInvalid]=useState(false);
 const change=useRef(onChange);change.current=onChange;
 useEffect(()=>{
  if(!recording||disabled)return;
  const cancel=()=>{setRecording(false);setInvalid(false)};
  const down=(event:KeyboardEvent)=>{
   event.preventDefault();event.stopImmediatePropagation();
   if(event.repeat||event.isComposing)return;
   if(event.code==="Escape"){cancel();return;}
   const key=optiscalerMenuKey(event);
   if(!key){setInvalid(true);return;}
   cancel();change.current(key);
  };
  window.addEventListener("keydown",down,true);
  window.addEventListener("blur",cancel);
  return()=>{window.removeEventListener("keydown",down,true);window.removeEventListener("blur",cancel)};
 },[recording,disabled]);
 useEffect(()=>{if(disabled){setRecording(false);setInvalid(false)}},[disabled]);
 return <div className="upscaler-menu-key">
  <div className="upscaler-shortcut-copy"><strong>{t("upscaler","menu_key")}</strong><small>{t("upscaler","key_description")}</small></div>
  <div className="upscaler-shortcut-actions"><div className="upscaler-shortcut-buttons"><button type="button" className="modern-mini-btn" disabled={disabled} data-shortcut-recording={recording||undefined} aria-label={`${t("upscaler","menu_key")}: ${optiscalerMenuKeyLabel(value)}`} aria-pressed={recording}
    onClick={()=>{setInvalid(false);setRecording(!recording)}} onBlur={()=>{setRecording(false);setInvalid(false)}}>{recording?t("upscaler","key_recording"):optiscalerMenuKeyLabel(value)}</button>
   <button type="button" className="modern-mini-btn" title={t("upscaler","key_restore")} disabled={disabled||recording||optiscalerMenuKeyLabel(value)==="Insert"} onClick={()=>onChange("0x2D")}>{t("upscaler","key_default")}</button></div>
   <small role="status">{t("upscaler",invalid?"key_unsupported":recording?"key_record_hint":"key_hint")}</small></div>
 </div>;
}

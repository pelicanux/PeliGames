import {useEffect,useSyncExternalStore} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {toggleMatrixMicrophone,toggleMatrixDeafen} from "./matrixVoice";
import {voiceShortcut,validateVoiceShortcuts,emptyVoiceShortcuts,type VoiceShortcuts,type VoiceShortcutAction} from "./matrixVoiceShortcutKeys";
const KEY="peligames.matrix.voiceShortcuts";
function load():VoiceShortcuts{try{const saved=JSON.parse(localStorage.getItem(KEY)||"{}");const value={microphone:typeof saved.microphone==="string"?saved.microphone:"",deafen:typeof saved.deafen==="string"?saved.deafen:""};return validateVoiceShortcuts(value)?{...emptyVoiceShortcuts}:value;}catch{return {...emptyVoiceShortcuts};}}
let state={preferences:load(),global:false,busy:false,error:"",triggers:{} as Record<string,string>};
const listeners=new Set<()=>void>();let initialized=false;
const set=(patch:Partial<typeof state>)=>{state={...state,...patch};listeners.forEach(fn=>fn());};
export function useMatrixShortcutPreferences(){return useSyncExternalStore(fn=>{listeners.add(fn);return()=>listeners.delete(fn);},()=>state);}
async function bind(preferences:VoiceShortcuts){
  if("__PELI_UI_PREVIEW__" in window){set({global:false,error:"",triggers:{}});return;}
  try{const triggers=await invoke<Record<string,string>>("configure_matrix_voice_shortcuts",{preferences});set({global:Boolean(preferences.microphone||preferences.deafen),triggers,error:""});}
  catch(error){set({global:false,triggers:{},error:`Atalhos disponíveis apenas com o launcher em foco. ${String(error)}`});}
}
export async function saveMatrixVoiceShortcuts(preferences:VoiceShortcuts){
  const error=validateVoiceShortcuts(preferences);if(error)throw Error(error);
  if(state.busy)return;set({busy:true,error:""});
  try{await bind(preferences);localStorage.setItem(KEY,JSON.stringify(preferences));set({preferences:{...preferences}});}finally{set({busy:false});}
}
const run=(action:VoiceShortcutAction)=>{if(document.querySelector('[data-shortcut-recording="true"]'))return;void(action==="microphone"?toggleMatrixMicrophone():toggleMatrixDeafen());};
export function useMatrixVoiceShortcuts(){
  useEffect(()=>{
    let alive=true;
    if(!initialized){initialized=true;if(state.preferences.microphone||state.preferences.deafen)void bind(state.preferences);}
    const native="__PELI_UI_PREVIEW__" in window?null:listen<string>("matrix-voice-shortcut",event=>{if(alive&&state.global&&["microphone","deafen"].includes(event.payload))run(event.payload as VoiceShortcutAction);}).catch(()=>()=>{});
    const unavailable="__PELI_UI_PREVIEW__" in window?null:listen<string>("matrix-voice-shortcuts-unavailable",event=>{if(alive)set({global:false,triggers:{},error:`Atalhos disponíveis apenas com o launcher em foco. ${event.payload}`});}).catch(()=>()=>{});
    const keydown=(event:KeyboardEvent)=>{
      if(state.global||event.repeat||event.isComposing||document.querySelector('[data-shortcut-recording="true"]')||(event.target instanceof Element&&event.target.closest('input,textarea,[contenteditable="true"]')))return;
      const key=voiceShortcut(event);if(!key)return;
      const action=(["microphone","deafen"] as const).find(action=>state.preferences[action]===key);
      if(action){event.preventDefault();run(action);}
    };
    window.addEventListener("keydown",keydown);
    return()=>{alive=false;window.removeEventListener("keydown",keydown);void native?.then(stop=>stop());void unavailable?.then(stop=>stop());};
  },[]);
}

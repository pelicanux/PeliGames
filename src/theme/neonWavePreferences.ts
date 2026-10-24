import {useSyncExternalStore} from "react";
export const NEON_WAVE_KEY="peligames.neonWave";
export type NeonWaveOptions={audio:boolean;intensity:number;reaction:number;demo:boolean};
export function readNeonWave(value:string|null):NeonWaveOptions {
  try{const data=JSON.parse(value||"{}");return {audio:data.audio!==false,intensity:typeof data.intensity==="number"&&Number.isFinite(data.intensity)?Math.max(10,Math.min(100,Math.round(data.intensity))):55,reaction:typeof data.reaction==="number"&&Number.isFinite(data.reaction)?Math.max(0,Math.min(100,Math.round(data.reaction))):40,demo:data.demo!==false};}catch{return {audio:true,intensity:55,reaction:40,demo:true};}
}
let options=(()=>{try{return readNeonWave(localStorage.getItem(NEON_WAVE_KEY));}catch{return readNeonWave(null);}})();
const listeners=new Set<()=>void>();
export function setNeonWaveOptions(patch:Partial<NeonWaveOptions>){options=readNeonWave(JSON.stringify({...options,...patch}));try{localStorage.setItem(NEON_WAVE_KEY,JSON.stringify(options));}catch{/* Session-only settings remain usable. */}listeners.forEach(fn=>fn());}
window.addEventListener("storage",event=>{if(event.key===NEON_WAVE_KEY){options=readNeonWave(event.newValue);listeners.forEach(fn=>fn());}});
export function useNeonWaveOptions(){return useSyncExternalStore(fn=>{listeners.add(fn);return()=>listeners.delete(fn);},()=>options);}
export type NeonWaveAudioState="off"|"connecting"|"listening"|"unavailable"|"paused"|"demo";
let audioState:{status:NeonWaveAudioState;error:string}={status:"off",error:""};
const audioListeners=new Set<()=>void>();
export function setNeonWaveAudioState(status:NeonWaveAudioState,error=""){if(audioState.status===status&&audioState.error===error)return;audioState={status,error};audioListeners.forEach(fn=>fn());}
export function useNeonWaveAudioState(){return useSyncExternalStore(fn=>{audioListeners.add(fn);return()=>audioListeners.delete(fn);},()=>audioState);}

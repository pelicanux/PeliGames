import {useEffect,useSyncExternalStore} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {validRGB,type RGB} from './openrgbColor';
export type RGBDevice={key:string;name:string;color:RGB|null;zones:{name:string;color:RGB|null}[]};
type Preferences={enabled:boolean;port:number;device:string;zone:number};
const KEY='peligames.openrgb';
function load():Preferences{try{const v=JSON.parse(localStorage.getItem(KEY)||'{}');return {enabled:v.enabled===true,port:Number.isInteger(v.port)&&v.port>0&&v.port<=65535?v.port:6742,device:typeof v.device==='string'?v.device:'',zone:Number.isInteger(v.zone)&&v.zone>=-1?v.zone:-1};}catch{return {enabled:false,port:6742,device:'',zone:-1};}}
let state={preferences:load(),devices:[] as RGBDevice[],color:null as RGB|null,status:'off' as 'off'|'connecting'|'connected'|'error',error:'',busy:false,demoColor:[255,65,170] as RGB};
const subscribers=new Set<()=>void>();
function set(patch:Partial<typeof state>){state={...state,...patch};subscribers.forEach(fn=>fn());}
export function useOpenRGB(){return useSyncExternalStore(fn=>{subscribers.add(fn);return()=>subscribers.delete(fn);},()=>state);}
function save(preferences:Preferences){try{localStorage.setItem(KEY,JSON.stringify(preferences));}catch{/* Keep settings for this session. */}}
export function configureOpenRGB(patch:Partial<Preferences>){const next={...state.preferences,...patch};save(next);set({preferences:next,color:null,error:'',status:next.enabled?'connecting':'off'});}
export function setOpenRGBDemoColor(color:RGB){if(validRGB(color))set({demoColor:color});}
let requesting=false;
async function read(){
 if(requesting||!state.preferences.enabled)return;
 requesting=true;set({busy:true});const preferences=state.preferences;
 try{
 const devices='__PELI_UI_PREVIEW__' in window?[{key:'preview-case',name:'Gabinete RGB (simulação)',color:state.demoColor,zones:[{name:'Ventoinhas',color:state.demoColor},{name:'Fita LED',color:[65,210,255] as RGB}]}]:await invoke<RGBDevice[]>('read_openrgb',{port:preferences.port});
 if(state.preferences!==preferences)return;
 if(!preferences.device&&devices.length){const next={...preferences,device:devices[0].key};save(next);set({preferences:next});}
 const device=devices.find(d=>d.key===state.preferences.device),zone=state.preferences.zone;
 const color=zone>=0?device?.zones[zone]?.color:device?.color;
 const nextColor=validRGB(color)?color:null;const stableColor=nextColor&&state.color&&nextColor.every((n,i)=>n===state.color![i])?state.color:nextColor;
 set({devices,color:stableColor,status:'connected',error:!devices.length?'Nenhum dispositivo RGB encontrado.':!device?'Dispositivo selecionado indisponível.':!validRGB(color)?'A zona selecionada não informou uma cor.':''});
 }catch(error){if(state.preferences===preferences)set({color:null,devices:[],status:'error',error:String(error)});}
 finally{requesting=false;set({busy:false});}
}
export function refreshOpenRGB(){return read();}
export function useOpenRGBConnection(){
 const {preferences}=useOpenRGB();
 useEffect(()=>{
  let cancelled=false,timer:ReturnType<typeof setTimeout>;
  const poll=async()=>{await read();if(!cancelled)timer=setTimeout(poll,state.status==='error'||document.hidden?5000:1000);};
  if(preferences.enabled)void poll();else{set({color:null,status:'off',error:'',devices:[]});if(!('__PELI_UI_PREVIEW__' in window))void invoke('disconnect_openrgb').catch(()=>{});}
  return()=>{cancelled=true;clearTimeout(timer);};
 },[preferences.enabled,preferences.port,preferences.device,preferences.zone]);
 return useOpenRGB();
}

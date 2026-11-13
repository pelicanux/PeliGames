import {createLatestWriteQueue} from "../../../_infra/ui-sdk/helpers/latestWriteQueue";
import type {UpscalerSettings} from "./settings";
export function createUpscalerChoice(initial:UpscalerSettings,persist?: (key:string,value:string)=>Promise<unknown>){
 let confirmed={...initial};let state={values:{...initial},busy:false,error:"",saved:false};const listeners=new Set<()=>void>();
 const publish=(next:typeof state)=>{state=next;listeners.forEach(fn=>fn());};
 const queue=createLatestWriteQueue<UpscalerSettings,void>(async values=>{
  for(const [key,value] of Object.entries(values))if(confirmed[key]!==value){await persist?.(key,value);confirmed={...confirmed,[key]:value};}
 },(_,__,error)=>publish({values:{...confirmed},busy:false,error:error===undefined?"":String(error),saved:error===undefined&&Boolean(persist)}));
 return {getSnapshot:()=>state,subscribe:(fn:()=>void)=>{listeners.add(fn);return()=>{listeners.delete(fn);};},
  change:(key:string,value:string)=>{const values={...state.values,[key]:value};publish({values,busy:true,error:"",saved:false});queue.enqueue(values);}};
}

import manifest from "../../addons/dlssnr-mochizuki/manifest.json";
import type {AddonManifest} from "../../src/modules/addons/contracts";
import type {PreviewAddon} from "../addons/preview";
import type {Runtime,GameState} from "../../addons/dlssnr-mochizuki/ui/src/api";
export function createMochiPreview():PreviewAddon & {installed:(directory:string)=>boolean}{
 let ready=true,models=true,next=0;
 const jobs=new Map<number,unknown>(),games=new Map<string,GameState>();
 const runtime=():Runtime=>({directory:"/preview/components/Mods/DLSSNR/backend-files/rdna4",current:ready?"v0.0.8.1-rdna4":null,ready,models,architectures:ready?["64","32"]:[]});
 const state=(directory:string):GameState=>games.get(directory)||{installation:{status:"OptiScaler",installed_dll:"dxgi.dll",dlssnr_installed:true},settings:{enabled:true,renodx:false},bits:"64",original:models,small:models,experimental:true,optimized:true,lumenite:true,optimized_layers:3,selected_network:"CC_SMALL_FP8",ready};
 const job=(result:unknown)=>{const id=++next;jobs.set(id,{id,done:true,result});return {job:id}};
 return {manifest:manifest as AddonManifest,installed:directory=>state(directory).installation.dlssnr_installed,call(method,request){
  switch(method){
   case "runtime.inspect":return runtime();
   case "runtime.check":return job("v0.0.8.1");
   case "runtime.download":case "runtime.import":ready=true;return job(runtime());
   case "model.extract":case "model.import":models=true;return job(runtime());
   case "runtime.remove":ready=false;models=false;return runtime();
   case "job.inspect":return jobs.get(Number(request.id));
   case "game.inspect":return state(String(request.directory));
   case "game.mutate":{
    const directory=String(request.directory),value=structuredClone(state(directory)),action=request.action as Record<string,unknown>;
    if(action.operation==="remove"){value.installation={status:"Não Instalado",installed_dll:null,dlssnr_installed:false};value.settings=null;}
    if(action.operation==="install"){value.installation={status:String(action.route),installed_dll:"dxgi.dll",dlssnr_installed:true};value.settings={enabled:Boolean(action.enabled),renodx:Boolean(action.renodx)};}
    if(action.operation==="startup")value.settings={enabled:Boolean(action.enabled),renodx:Boolean(action.renodx)};
    if(action.operation==="optimized")value.optimized_layers=Number(action.level);
    games.set(directory,value);return job('Steam launch options: WINEDLLOVERRIDES="dxgi=n,b" %command%');
   }
   default:throw Error(`Unsupported NR preview method: ${method}`);
  }
 }};
}

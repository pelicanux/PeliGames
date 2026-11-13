import manifest from "../../addons/afmf/manifest.json";
import type {AddonManifest} from "../../src/modules/addons/contracts";
import type {PreviewAddon} from "../addons/preview";
import type {Runtime,Settings} from "../../addons/afmf/ui/src/api";

/** In-memory demonstration, never reads a native installation or downloads. */
export function createAfmfPreview():PreviewAddon {
 let versions:string[]=["1.3.1"],current="1.3.1",nextJob=0;
 const jobs=new Map<number,unknown>();
 const directory=(version:string)=>`/preview/config/PeliGames/components/FrameGeneration/afmf/versions/${version}`;
 const state=():Runtime=>({current,latest:"1.3.1",available:false,compatible:true,ready:versions.includes(current),directory:directory(current),versions:versions.map(version=>({version,current:version===current,compatible:true,directory:directory(version)}))});
 const done=()=>{const id=++nextJob;jobs.set(id,{id,done:true,result:state(),progress:{phase:"complete",downloaded:100,total:100}});return {job:id}};
 return {manifest:manifest as AddonManifest,call(method,request){switch(method){
  case "runtime.inspect":return state();
  case "config.defaults":return {enabled:false,performance_mode:"auto",search_mode:"auto",fast_motion:"blend",min_fps:30,hud_detect:true,pacing:true} satisfies Settings;
  case "config.validate":return structuredClone(request);
  case "game.support":return {available:!request.flatpak,code:request.flatpak?"flatpak":"supported"};
  case "runtime.check":return done();
  case "runtime.download":if(!versions.includes("1.3.1"))versions.push("1.3.1");current="1.3.1";return done();
  case "runtime.select":if(!versions.includes(String(request.version)))throw Error("Missing preview version");current=String(request.version);return state();
  case "runtime.remove":versions=versions.filter(version=>version!==request.version);current=versions[0]??"1.3.1";return state();
  case "job.inspect":if(!jobs.has(Number(request.id)))throw Error("Unknown preview job");return jobs.get(Number(request.id));
  default:throw Error(`Unsupported AFMF preview method: ${method}`);
 }}};
}

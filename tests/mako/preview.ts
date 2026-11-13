import manifest from '../../addons/mako/manifest.json';
import type {AddonManifest} from '../../src/modules/addons/contracts';
import type {PreviewAddon} from '../addons/preview';
import type {Settings,State} from '../../addons/mako/ui/src/api';
export const defaults:Settings={backend:'mako',runtime_version:'4.5.0',enabled:false,overlay_enabled:false,flatpak:false,dll:'',multiplier:2,adaptive:false,target_fps:60,flow_scale:1,performance:false,gpu:'',base_fps_cap:0,refresh_threshold:0,ultra_performance:false,allow_fp16:true,live_enabled:true,scaling_enabled:false,scaling_method:'native',scaling_factor:1.5,scaling_sharpness:.8};
export function createMakoPreview():PreviewAddon{
 let installed=true,next=0;const jobs=new Map<number,unknown>(),profiles=new Map<string,Settings>();
 const directory='/preview/config/PeliGames/components/FrameGeneration/mako/versions/4.5.0';
 const state=(profile?:string):State=>({settings:structuredClone(profile&&profiles.get(profile)||defaults),profile:profile||null,installed,directory,versions:installed?[{version:'4.5.0',current:true,in_use:[...profiles.values()].some(s=>s.enabled),directory}]:[]});
 const done=(result:unknown)=>{const id=++next;jobs.set(id,{id,done:true,result,progress:{phase:'complete',downloaded:100,total:100}});return {job:id}};
 return {manifest:manifest as AddonManifest,call(method,request){switch(method){
  case 'runtime.inspect':return state();
  case 'game.inspect':return state((request.profiles as string[]).find(p=>profiles.has(p)));
  case 'runtime.check':return done({version:'4.5.0',compatible:true,available:false});
  case 'runtime.download':installed=true;return done(null);
  case 'runtime.remove':if([...profiles.values()].some(s=>s.enabled))throw Error('Runtime in use');installed=false;return null;
  case 'runtime.remove-backup':return null;
  case 'dll.detect':return '/preview/Steam/steamapps/common/Lossless Scaling/Lossless.dll';
  case 'flatpak.inspect':return {ready:false,message:'Preview: install the matching Flatpak extension.',commands:[]};
  case 'config.prepare':{const path=String(request.profile||'/preview/config/PeliGames/components/FrameGeneration/profiles/game.sh'),settings=structuredClone(request.settings as Settings);profiles.set(path,settings);return {...settings,profile:path,game_key:request.key,game_name:request.label,directory:request.directory}}
  case 'config.live':profiles.set(String(request.profile),structuredClone(request.settings as Settings));return null;
  case 'config.defaults':return structuredClone(defaults);
  case 'config.validate':return request;
  case 'job.inspect':if(!jobs.has(Number(request.id)))throw Error('Unknown preview job');return jobs.get(Number(request.id));
  default:throw Error(`Unsupported MAKO preview method: ${method}`);
 }}};
}

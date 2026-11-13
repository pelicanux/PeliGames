import manifest from '../../addons/gpu-screen-recorder/manifest.json';
import type {AddonManifest} from '../../src/modules/addons/contracts';
import type {PreviewAddon} from '../addons/preview';
import {defaults,type Options,type Status} from '../../addons/gpu-screen-recorder/ui/src/api';
/** Simulates recorder IPC only; never records a desktop or writes a game. */
export function createCapturePreview(changed:(status:Status)=>void=()=>{}):PreviewAddon{
 let installed=true,next=0,directory='/preview/Videos/PeliGames';const jobs=new Map<number,unknown>(),settings=new Map<string,Options>();
 let shortcuts={recording:'Alt_L+F6',replay:'Alt_L+F7',save_replay:'Alt_L+F8'},current:Options={...defaults},status:Status={phase:'idle',recording:false,replay:false,game_key:null,last_file:null,error:null};
 const done=(result:unknown)=>{const id=++next;jobs.set(id,{id,done:true,result,progress:{phase:'complete',downloaded:100,total:100}});return {job:id}};
 return {manifest:manifest as AddonManifest,call(method,p){switch(method){
  case 'runtime.inspect':return {installed,version:'6.1.4',directory:'/preview/config/PeliGames/components/GPU-Screen-Recorder',bytes:50000000};
  case 'support.inspect':return {installed,ipc:installed,portal:true,monitors:['DP-1','HDMI-A-1']};
  case 'encoders.inspect':return {gpu:'Preview Radeon',codecs:['h264','hevc','av1'],containers:['mp4','mkv','webm']};
  case 'audio.inspect':return {outputs:[],inputs:[],applications:[]};
  case 'storage.inspect':return {directory,default_directory:'/preview/Videos/PeliGames'};
  case 'storage.configure':directory=String(p.directory||'/preview/Videos/PeliGames');return directory;
  case 'runtime.download':installed=true;return done(null);
  case 'runtime.remove':installed=false;return null;
  case 'job.inspect':return jobs.get(Number(p.id));
  case 'shortcuts.read':return structuredClone(shortcuts);
  case 'shortcuts.save':shortcuts=structuredClone(p.preferences as typeof shortcuts);return null;
  case 'settings.read':return settings.get(String(p.gameKey))||null;
  case 'selection.read':return settings.get(String(p.gameKey))||null;
  case 'settings.save':current=structuredClone(p.options as Options);settings.set(current.game_key,current);return null;
  case 'session.status':return structuredClone(status);
  case 'session.start':current=structuredClone(p.options as Options);status={...status,phase:'active',game_key:current.game_key,game_name:current.game_name,recording:!p.replay,replay:Boolean(p.replay)};changed(structuredClone(status));return done(structuredClone(status));
  case 'session.action':if(p.action==='stop')status={...status,phase:'idle',recording:false,replay:false};else if(p.action==='save-replay')status={...status,last_file:`${directory}/preview-replay.mkv`};else status={...status,recording:p.action==='start-recording'};changed(structuredClone(status));return done(structuredClone(status));
  case 'events.poll':return [];
  case 'clips.list':return [];
  case 'clips.folder':return directory;
  default:throw Error(`Unsupported capture preview method: ${method}`);
 }}};
}

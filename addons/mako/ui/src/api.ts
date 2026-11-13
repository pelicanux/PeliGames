import type {AddonHost} from '../../../_infra/ui-sdk';
import {waitForAddonJob} from '../../../_infra/ui-sdk/helpers/jobs';
import type {TransferProgress} from '../../../_infra/ui-sdk/helpers/downloadProgress';
import en from '../locales/en.json';
import pt from '../locales/pt-BR.json';
export interface Settings extends Record<string,unknown>{backend:string;runtime_version:string;enabled:boolean;overlay_enabled:boolean;flatpak:boolean;dll:string;multiplier:number;adaptive:boolean;target_fps:number;flow_scale:number;performance:boolean;gpu:string;base_fps_cap:number;refresh_threshold:number;ultra_performance:boolean;allow_fp16:boolean;live_enabled:boolean;scaling_enabled:boolean;scaling_method:string;scaling_factor:number;scaling_sharpness:number}
export interface State{settings:Settings;profile:string|null;installed:boolean;directory:string;versions:{version:string;current:boolean;in_use:boolean;directory:string}[]}
export interface Saved extends Settings{profile:string;game_key:string;game_name:string;directory:string}
export interface Sandbox{ready:boolean;message:string;commands:string[]}
export type Message=keyof typeof en;
export function connect(host:AddonHost){
 const t=host.i18n.createTranslator({defaultLanguage:'en',resources:{en,'pt-BR':pt}});
 const call=<T,>(method:string,request:unknown=null)=>host.service.call<T>(method,request);
 const job=async(method:string,request:unknown,progress?:(p:TransferProgress)=>void)=>{const {job}=await call<{job:number}>(method,request);return waitForAddonJob<unknown>(host,job,progress)};
 return {t:(key:Message)=>t(key),
  state:()=>call<State>('runtime.inspect'),
  inspect:(key:string,label:string,profiles:string[])=>call<State>('game.inspect',{key,label,profiles}),
  check:()=>job('runtime.check',null),download:(version:string,progress:(p:TransferProgress)=>void)=>job('runtime.download',{version},progress),
  remove:()=>call<void>('runtime.remove'),removeBackup:(version:string)=>call<void>('runtime.remove-backup',{version}),
  detect:(libraries:string[])=>call<string|null>('dll.detect',{libraries}),
  prepare:(key:string,label:string,directory:string,settings:Settings,profile:string|null,options:string[])=>call<Saved>('config.prepare',{key,label,directory,settings,profile,options}),
  live:(profile:string,settings:Settings)=>call<void>('config.live',{profile,settings}),
  sandbox:(key:string,dll:string,version:string)=>call<Sandbox>('flatpak.inspect',{key,dll,version}),
 };
}
export type Connection=ReturnType<typeof connect>;

import type {AddonHost} from '../../../_infra/ui-sdk';
import {waitForAddonJob} from '../../../_infra/ui-sdk/helpers/jobs';
import type {TransferProgress} from '../../../_infra/ui-sdk/helpers/downloadProgress';
import en from '../locales/en.json';import pt from '../locales/pt-BR.json';
export interface Options extends Record<string,unknown>{game_key:string;game_name:string;game_directory:string;game_executable:string;steam_app_id:string;method:string;monitor:string;replay_seconds:number;replay_on_launch:boolean;format:string;codec:string;resolution:string;fps:number;rate_control:string;bitrate_kbps:number;quality_preset:string;desktop_audio:boolean;microphone:boolean;microphone_noise_reduction:boolean;output_source:string;input_source:string}
export const defaults:Options={game_key:'',game_name:'',game_directory:'',game_executable:'',steam_app_id:'',method:'portal',monitor:'',replay_seconds:30,replay_on_launch:false,format:'mkv',codec:'auto',resolution:'1280x720',fps:60,rate_control:'qp',bitrate_kbps:5000,quality_preset:'high',desktop_audio:true,microphone:false,microphone_noise_reduction:false,output_source:'game:auto',input_source:'default_input'};
export interface Status{phase:string;game_key:string|null;game_name?:string;recording:boolean;replay:boolean;last_file:string|null;error:string|null}
export interface Runtime{installed:boolean;version:string;directory:string;bytes:number}
export interface Support{installed:boolean;ipc:boolean;portal:boolean;monitors:string[]}
export interface Sources{outputs:{id:string;label:string}[];inputs:{id:string;label:string}[];applications:{id:string;label:string}[]}
export interface Encoder{gpu:string;codecs:string[];containers:string[]}
export interface Clip{name:string;title:string;path:string;bytes:number;modified:number}
export type Message=keyof typeof en;
export function connect(host:AddonHost){const translate=host.i18n.createTranslator({defaultLanguage:'en',resources:{en,'pt-BR':pt}});const call=<T,>(method:string,params:unknown=null)=>host.service.call<T>(method,params);
 const operation=async<T,>(method:string,params:unknown=null,progress?:(p:TransferProgress)=>void)=>{const result=await call<T|{job:number}>(method,params);return result&&typeof result==='object'&&'job'in result?await waitForAddonJob<T>(host,result.job,progress):result as T};
 return {t:(key:Message)=>translate(key),call,operation};}
export type Api=ReturnType<typeof connect>;

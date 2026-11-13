import type {AddonHost} from "../../../_infra/ui-sdk";
import {waitForAddonJob} from "../../../_infra/ui-sdk/helpers/jobs";
import type {TransferProgress} from "../../../_infra/ui-sdk/helpers/downloadProgress";
import en from "../locales/en.json";
import pt from "../locales/pt-BR.json";

export interface Settings extends Record<string,unknown> {enabled:boolean;performance_mode:"auto"|"quality"|"performance";search_mode:"auto"|"standard"|"high";fast_motion:"blend"|"repeat";min_fps:number;hud_detect:boolean;pacing:boolean}
export interface Runtime {current:string;latest:string;available:boolean;compatible:boolean;ready:boolean;directory:string;versions:{version:string;current:boolean;directory:string;compatible:boolean}[]}
export type Message=keyof typeof en;
export function connect(host:AddonHost){
 const translate=host.i18n.createTranslator({defaultLanguage:"en",resources:{en,"pt-BR":pt}});
 const call=<T,>(method:string,params:unknown=null)=>host.service.call<T>(method,params);
 const job=async(method:string,params:unknown,progress?:(value:TransferProgress)=>void)=>{
  const operation=await call<{job:number}>(method,params);const result=await waitForAddonJob<Runtime>(host,operation.job,progress);if(!result)throw new Error("Missing AFMF operation result");return result;
 };
 return {
  t:(key:Message)=>translate(key),
  state:()=>call<Runtime>("runtime.inspect"),defaults:()=>call<Settings>("config.defaults"),
  validate:(settings:Settings)=>call<Settings>("config.validate",settings),
  support:(value:unknown)=>call<{available:boolean;code:Message}>("game.support",value),
  check:()=>job("runtime.check",{force:true}),download:(version:string,progress:(value:TransferProgress)=>void)=>job("runtime.download",{version},progress),
  select:(version:string)=>call<Runtime>("runtime.select",{version}),remove:(version:string)=>call<Runtime>("runtime.remove",{version}),
 };
}
export type Connection=ReturnType<typeof connect>;

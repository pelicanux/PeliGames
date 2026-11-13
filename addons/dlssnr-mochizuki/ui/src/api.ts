import type {AddonHost} from "../../../_infra/ui-sdk";
import {waitForAddonJob} from "../../../_infra/ui-sdk/helpers/jobs";
import type {TransferProgress} from "../../../_infra/ui-sdk/helpers/downloadProgress";
import en from "../locales/en.json";
import pt from "../locales/pt-BR.json";
export interface Runtime{directory:string;current:string|null;ready:boolean;models:boolean;architectures:string[]}
export interface GameState{installation:{status:string;installed_dll:string|null;dlssnr_installed:boolean};settings:{enabled:boolean;renodx:boolean}|null;bits:string;original:boolean;small:boolean;experimental:boolean;optimized:boolean;lumenite:boolean;optimized_layers:number|null;selected_network:string|null;ready:boolean;route?:string|null;shortcut?:string;lumenite_selected?:boolean}
export function connect(host:AddonHost){
 const t=host.i18n.createTranslator({defaultLanguage:"en",resources:{en,"pt-BR":pt}});
 const call=<T,>(method:string,params:unknown=null)=>host.service.call<T>(method,params);
 const job=async<T,>(method:string,params:unknown=null,progress?:(p:TransferProgress)=>void)=>{
  const {job}=await call<{job:number}>(method,params);return await waitForAddonJob<T>(host,job,progress);
 };
 return {t:(key:keyof typeof en)=>t(key),call,job,state:()=>call<Runtime>("runtime.inspect"),inspect:(directory:string,bits:string)=>call<GameState>("game.inspect",{directory,bits})};
}
export type Api=ReturnType<typeof connect>;

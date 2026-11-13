import {optiscalerMenuKeyLabel} from "./menuShortcut";
export type UpscalerSettings = Record<string,string>;
export interface OptiScalerSoftware {id:"optiscaler"|"dlss"|"fsr"|"xess"|"fakenvapi"|"streamline"|"optipatcher";present:boolean;integrated:boolean;version:string|null}
export interface UpscalerState {software?:OptiScalerSoftware[];patcher?:{installed:boolean;enabled:boolean;managed:boolean};installed:boolean;managed:boolean;nr_route:"optiscaler"|"reshade"|"unknown"|null;component_ready:boolean;framegen_available:boolean;framegen_modern?:boolean;proxy:string|null;settings:UpscalerSettings}
export const upscalerFields = [
 {key:"Upscalers.Dx12Upscaler",label:"dx12",group:"upscaling",section:"upscaling",values:["auto","ffx","xess","fsr21","fsr22","dlss"]},
 {key:"Upscalers.Dx11Upscaler",label:"dx11",group:"upscaling",section:"upscaling",values:["auto","fsr22","fsr31","xess","xess_12","ffx_12","dlss"]},
 {key:"Upscalers.VulkanUpscaler",label:"vulkan",group:"upscaling",section:"upscaling",values:["auto","fsr21","fsr22","ffx","xess","ffx_12","dlss"]},
 {key:"FSR.UpscalerIndex",label:"fsr_version",group:"upscaling",section:"fsr_settings",values:["auto","0","1","2"]},
 {key:"FSR.Fsr4Update",label:"fsr4_update",group:"upscaling",section:"fsr_settings",values:["auto","true","false"]},
 {key:"FSR.Fsr4ForceEnableInt8",label:"fsr4_int8",group:"upscaling",section:"fsr_settings",values:["auto","true","false"]},
 {key:"FSR.Fsr4GpuSupport",label:"fsr4_gpu_support",group:"upscaling",section:"fsr_settings",values:["auto","0","1","2","3","4"]},
 {key:"FSR.Fsr4Model",label:"fsr4_model",group:"upscaling",section:"fsr_settings",values:["auto","0","1","2","3"]},
 {key:"FrameGen.Enabled",label:"fg_enabled",group:"framegeneration",section:"framegeneration",values:["false","true"]},
 {key:"FrameGen.FGInput",label:"fg_input",group:"framegeneration",section:"framegeneration",values:["nofg","dlssg","nvngxfg","fsrfg","upscaler","fsrfg30","xefg"]},
 {key:"FrameGen.FGOutput",label:"fg_output",group:"framegeneration",section:"framegeneration",values:["nofg","fsrfg","xefg","dlssg"]},
 {key:"FrameGen.FGNvngxReplacement",label:"fg_provider",group:"framegeneration",section:"framegeneration",values:["None","Nukems","FFX"]},
 {key:"Menu.Scale",label:"menu_scale",group:"main",section:"menu_settings",values:["auto","0.75","1","1.25","1.5","1.75","2"]},
 {key:"Menu.ShortcutKey",label:"menu_key",group:"main",section:"menu_settings",values:["auto"]},
 {key:"Inputs.EnableDlssInputs",label:"dlss_inputs",group:"main",section:"compatibility_settings",values:["auto","true","false"]},
 {key:"Spoofing.Dxgi",label:"gpu_spoofing",group:"main",section:"compatibility_settings",values:["auto","true","false"]},
 {key:"fakenvapi.ForceReflex",label:"reflex",group:"latency",section:"latency",values:["auto","0","1","2"]},
 {key:"fakenvapi.ForceLatencyFlex",label:"latencyflex",group:"latency",section:"latency",values:["auto","true","false"]},
 {key:"fakenvapi.LatencyFlexMode",label:"latency_mode",group:"latency",section:"latency",values:["auto","0","1","2"]},
] as const;
const upscalerToggleValues:Record<string,{on:string;off:string}>={
 "FrameGen.Enabled":{on:"true",off:"false"},
 "FSR.Fsr4Update":{on:"true",off:"false"},
 "FSR.Fsr4ForceEnableInt8":{on:"true",off:"false"},
 "Inputs.EnableDlssInputs":{on:"true",off:"false"},
 "Spoofing.Dxgi":{on:"true",off:"false"},
 "fakenvapi.ForceReflex":{on:"2",off:"1"},
 "fakenvapi.ForceLatencyFlex":{on:"true",off:"false"},
};
export function upscalerToggle(key:string,value:string){
 const states=upscalerToggleValues[key];
 return states?{enabled:value===states.on,inherited:value!==states.on&&value!==states.off,next:value===states.on?states.off:states.on}:null;
}
export function defaultUpscalerSettings(modern=true):UpscalerSettings {
 const values=Object.fromEntries(upscalerFields.map(field=>[field.key,"auto"]));
 if(modern){delete values["FSR.Fsr4Update"];delete values["FSR.Fsr4ForceEnableInt8"];}
 else {delete values["FSR.Fsr4GpuSupport"];delete values["FSR.Fsr4Model"];}
 return {...values,"Menu.ShortcutKey":"0x2D","FrameGen.Enabled":"false","FrameGen.FGInput":"nofg","FrameGen.FGOutput":"nofg","FrameGen.FGNvngxReplacement":"Nukems"};
}
/** Optional FSR controls are exposed only when documented by the installed INI. */
export function upscalerFieldAvailable(key:string,settings:UpscalerSettings){
 return !key.startsWith("FSR.Fsr4")||settings[key]!==undefined;
}
export function upscalerFieldValues(field:typeof upscalerFields[number],settings:UpscalerSettings):readonly string[]{
 const legacy=settings["FSR.Fsr4Update"]!==undefined&&settings["FSR.Fsr4GpuSupport"]===undefined;
 return legacy&&field.key.startsWith("Upscalers.")?field.values.map(value=>value==="ffx"?"fsr31":value==="ffx_12"?"fsr31_12":value):field.values;
}
/** Read old defaults without claiming we detected whether the game is generating frames. */
export function upscalerDisplayValue(_key:string,value:string):string {
 return value;
}

export function upscalerValueLabel(key:string,value:string,auto:string,on:string,off:string):string {
 if(value==="auto")return auto;
 if(value==="true")return on;if(value==="false")return off;
 if(value==="nofg"||value==="None")return off;
 if(key.startsWith("FrameGen."))return ({dlssg:"DLSSG",nvngxfg:"DLSSG via NGX",fsrfg:"FSR FG",fsrfg30:"FSR 3.0 FG",upscaler:"OptiFG",xefg:"XeFG",Nukems:"Nukem · FSR 3 FG",FFX:"FSR FG"} as Record<string,string>)[value]??value;
 if(key==="Menu.ShortcutKey")return optiscalerMenuKeyLabel(value);
 if(key==="Menu.Scale")return `${Math.round(Number(value)*100)}%`;
 if(key==="FSR.UpscalerIndex")return ({"0":"FSR 4","1":"FSR 3.1","2":"FSR 2.3"} as Record<string,string>)[value]??value;
 return ({ffx:"FSR",ffx_12:"FSR (DX12 bridge)",xess:"XeSS",xess_12:"XeSS (DX12 bridge)",fsr21:"FSR 2.1",fsr22:"FSR 2.2",fsr31:key==="Upscalers.Dx11Upscaler"?"FSR 3.1":"FSR",fsr31_12:"FSR (DX12 bridge)",dlss:"DLSS"} as Record<string,string>)[value]??value;
}

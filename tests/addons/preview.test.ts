import {expect,test} from "bun:test";
import {createAddonPreview} from "./preview";
import {createUpscalerPreview} from "../upscaler/preview";
import {createMakoPreview} from "../mako/preview";
import {createAfmfPreview} from "../afmf/preview";

test("browser preview uses external contributions and package removal preserves installed game copies",()=>{
 const files=createUpscalerPreview(()=>false);
 const preview=createAddonPreview(files,"https://preview.invalid");
 const initial=preview.overview();expect(initial.contributions.map(item=>item.contribution.target)).toEqual(["settings","game-mods"]);
 const descriptor=preview.invoke("addon_ui_load",{id:"optiscaler",target:"game-mods",route:"game"}) as {source:string};
 expect(descriptor.source).toContain("https://preview.invalid/addons/optiscaler/ui/src/index.tsx");
 const request=(action:unknown)=>preview.invoke("addon_service_call",{id:"optiscaler",method:"game.mutate",params:{directory:"/preview/game",action}});
 request({operation:"install",proxy:"dxgi.dll",settings:{}});
 request({operation:"update",key:"Menu.Scale",value:"1.25"});
 expect(files.state("/preview/game").settings["Menu.Scale"]).toBe("1.25");
 preview.invoke("addon_remove_version",{id:"optiscaler",version:"0.1.0"});
 expect(preview.overview().contributions).toEqual([]);expect(files.state("/preview/game").installed).toBe(true);
 expect(()=>preview.invoke("addon_ui_load",{id:"optiscaler",target:"game-mods",route:"game"})).toThrow();
});

test("AFMF preview loads from its package and its removal leaves the other add-on active",()=>{
 const preview=createAddonPreview(createUpscalerPreview(()=>false),"https://preview.invalid",[createAfmfPreview()]);
 const descriptor=preview.invoke("addon_ui_load",{id:"afmf",target:"game-frame-generation",route:"game"}) as {source:string};
 expect(descriptor.source).toContain("/addons/afmf/ui/src/index.tsx");
 const call=(method:string,params:unknown={})=>preview.invoke("addon_service_call",{id:"afmf",method,params}) as Record<string,unknown>;
 expect(call("runtime.inspect").ready).toBe(true);
 call("runtime.remove",{version:"1.3.1"});expect(call("runtime.inspect").ready).toBe(false);
 const {job}=call("runtime.download");expect(call("job.inspect",{id:job}).done).toBe(true);
 preview.invoke("addon_set_enabled",{id:"afmf",enabled:false});
 expect(preview.overview().contributions.every(item=>item.id==="optiscaler")).toBe(true);
 expect(()=>call("runtime.inspect")).toThrow();
 preview.invoke("addon_remove_version",{id:"afmf",version:"0.1.0"});
 expect(preview.overview().state.addons.afmf).toBeUndefined();
});

test("local preview import does not publish a catalog artifact",()=>{
 const preview=createAddonPreview(createUpscalerPreview(()=>false),"http://localhost");
 const plan=preview.invoke("addon_local_plan",{path:"/preview/experiment.zip"}) as {sha256:string};
 preview.invoke("addon_import_local",{path:"/preview/experiment.zip",sha256:plan.sha256});
 const overview=preview.overview();
 expect(overview.state.addons["local-experiment"].versions["0.1.0"].local).toBe(true);
 expect(overview.catalog.addons.some(entry=>entry.manifest.id==="local-experiment")).toBe(false);
});

test("MAKO preview is an independent package and its removal preserves the AFMF contribution",()=>{
 const preview=createAddonPreview(createUpscalerPreview(()=>false),'https://preview.invalid',[createAfmfPreview(),createMakoPreview()]);
 const descriptor=preview.invoke('addon_ui_load',{id:'mako',target:'game-frame-generation',route:'game'}) as {source:string};expect(descriptor.source).toContain('/addons/mako/ui/src/index.tsx');
 const state=preview.invoke('addon_service_call',{id:'mako',method:'runtime.inspect'}) as {installed:boolean};expect(state.installed).toBe(true);
 preview.invoke('addon_remove_version',{id:'mako',version:'0.1.0'});expect(preview.overview().contributions.some(c=>c.id==='mako')).toBe(false);expect(preview.overview().contributions.some(c=>c.id==='afmf')).toBe(true);
});

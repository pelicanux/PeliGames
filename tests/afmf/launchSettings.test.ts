import {expect,test} from "bun:test";
import {normalizeAdvanced} from "../../src/services/advancedSettings";
import {releaseAddonLaunchGroup} from "../../src/modules/addons/launchSettings";
import {addonLaunchData,applyAddonLaunchSettings,activeFrameGenerationAddons} from "../../src/modules/addons/launchSettings";
import type {AddonOverview} from "../../src/modules/addons/contracts";
import manifest from "../../addons/afmf/manifest.json";
const overview:AddonOverview={catalog:{addons:[]},platform:"linux-x86_64",host_version:"0.5.5",contributions:[],state:{selection_completed:true,addons:{afmf:{enabled:true,selected:manifest.version,versions:{[manifest.version]:{manifest:{...manifest,category:"frame-generation",contributions:[]},consumers:[]}}}}}};
test("legacy launch data migrates into the add-on while preserving tuning and core fields",()=>{
 const old=normalizeAdvanced();old.launch_options="KEEP=1 %command%";old.afmf={enabled:true,min_fps:45,search_mode:"high"};old.wrappers=[{program:"/config/FrameGeneration/profiles/game.sh",arguments:"",enabled:true},{program:"/other-wrapper",arguments:"literal",enabled:true}];
 const game={path:"/fixture/game",name:"Fixture",launcher:"PeliGames",advanced:old};
 expect(addonLaunchData(game,"afmf")).toEqual(old.afmf);
 const next=applyAddonLaunchSettings(game,"afmf","frame-generation",{enabled:true,min_fps:45,search_mode:"high"},overview);
 expect(next.afmf).toBeUndefined();expect(next.addon_launch?.afmf.data.min_fps).toBe(45);expect(next.launch_options).toBe(old.launch_options);
 expect(next.wrappers?.[0].enabled).toBe(false);expect(next.wrappers?.[1]).toEqual(old.wrappers[1]);expect(old.wrappers[0].enabled).toBe(true);
 const mako=releaseAddonLaunchGroup(next,"frame-generation");expect(mako.addon_launch?.afmf.enabled).toBe(false);expect(mako.addon_launch?.afmf.data).toEqual(next.addon_launch?.afmf.data);
 expect((addonLaunchData({...game,advanced:mako},"afmf") as {enabled:boolean}).enabled).toBe(false);
 const legacyMako=releaseAddonLaunchGroup(old,"frame-generation",overview);
 expect(legacyMako.afmf).toEqual({...old.afmf as object,enabled:false});expect(old.afmf).toEqual({enabled:true,min_fps:45,search_mode:"high"});
});
test("metadata and registry selection own visibility; disabled or missing add-ons never claim an active generator",()=>{
 const settings=normalizeAdvanced();settings.addon_launch={afmf:{enabled:true,group:"frame-generation",data:{enabled:true}}};
 expect(activeFrameGenerationAddons(settings,overview)).toEqual([{id:"afmf",name:"AFMF"}]);
 expect(activeFrameGenerationAddons(settings,{...overview,state:{...overview.state,addons:{}}})).toEqual([]);
 expect(activeFrameGenerationAddons(settings,{...overview,state:{...overview.state,addons:{afmf:{...overview.state.addons.afmf,enabled:false}}}})).toEqual([]);
});
test("disabling preserves other modules and rejects malformed or oversized launch data",()=>{
 const advanced=normalizeAdvanced();advanced.addon_launch={other:{enabled:true,group:"mods",data:{preserve:true}}};
 const game={path:"/fixture/game",name:"Fixture",launcher:"PeliGames",advanced};
 const next=applyAddonLaunchSettings(game,"afmf","frame-generation",{enabled:false,min_fps:60},overview);
 expect(next.addon_launch?.other).toEqual(advanced.addon_launch.other);expect(next.addon_launch?.afmf.enabled).toBe(false);
 expect(()=>applyAddonLaunchSettings(game,"afmf","frame-generation",{enabled:"yes"},overview)).toThrow();
 expect(()=>applyAddonLaunchSettings(game,"afmf","frame-generation",{enabled:true,data:"x".repeat(33*1024)},overview)).toThrow();
});

test("MAKO and AFMF release each other through the same opaque group",()=>{
 const old=normalizeAdvanced();old.addon_launch={afmf:{enabled:true,group:"frame-generation",data:{enabled:true,min_fps:45}}};
 const game={path:"/fixture/game",name:"Fixture",launcher:"PeliGames",advanced:old};
 const mako=applyAddonLaunchSettings(game,"mako","frame-generation",{enabled:true,profile:"/fixture/game.sh"},overview);
 expect(mako.addon_launch!.afmf.enabled).toBe(false);expect(mako.addon_launch!.afmf.data.min_fps).toBe(45);
 const afmf=applyAddonLaunchSettings({...game,advanced:mako},"afmf","frame-generation",{enabled:true,min_fps:45},overview);
 expect(afmf.addon_launch!.mako.enabled).toBe(false);expect(afmf.addon_launch!.mako.data.profile).toBe("/fixture/game.sh");
 expect(old.addon_launch.afmf.enabled).toBe(true);
});

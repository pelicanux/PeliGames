import {expect,test} from "bun:test";
import {createAddonStore,type AddonProgress,type StoreBridge} from "../../src/modules/addons/store";
import {storeEntries,visibleGameTabs,manifestCategory,gameContributions} from "../../src/modules/addons/navigation";
import type {AddonManifest,AddonOverview,UiContribution} from "../../src/modules/addons/contracts";

const manifest:AddonManifest={id:"example",name:"Example",version:"1.0.0",summary:"Example integration",repository:"https://example.com",contributions:[],dependencies:{}};
const overview=():AddonOverview=>({catalog:{addons:[]},state:{selection_completed:true,addons:{}},platform:"linux-x86_64",host_version:"0.5.5",contributions:[]});
function deferred<T>(){let resolve!:(value:T)=>void;let reject!:(error:unknown)=>void;const promise=new Promise<T>((yes,no)=>{resolve=yes;reject=no});return {resolve,reject,promise}}
const flush=async()=>{for(let i=0;i<30;i++)await Promise.resolve()};
class Bridge implements StoreBridge {
 value=overview();events=new Set<()=>void>();calls:[string,Record<string,unknown>][]=[];progress?: (value:AddonProgress)=>void;
 reading=async()=>this.value;
 work=async(_name:string,_args:Record<string,unknown>):Promise<unknown>=>{};
 async read(){return this.reading()}
 async observe(fn:()=>void){this.events.add(fn);return()=>{this.events.delete(fn)}}
 async command<T>(name:string,args:Record<string,unknown>){this.calls.push([name,args]);return await this.work(name,args) as T}
 async install(id:string,progress:(value:AddonProgress)=>void){this.progress=progress;return this.command("download",{id})}
 emit(){for(const fn of this.events)fn()}
}
test("store closes the initial read/observer race and rejects stale reads",async()=>{
 const bridge=new Bridge();const waiting=deferred<AddonOverview>();let reads=0;bridge.reading=()=>++reads===1?waiting.promise:Promise.resolve(bridge.value);
 const store=createAddonStore(bridge);const release=store.subscribe(()=>{});await flush();
 expect(store.getSnapshot().overview).toBe(bridge.value);waiting.resolve({...overview(),host_version:"old"});await flush();
 expect(store.getSnapshot().overview?.host_version).toBe("0.5.5");release();expect(bridge.events.size).toBe(0);
});
test("unmounting releases a late observer and invalidates pending refresh",async()=>{
 const bridge=new Bridge(),registration=deferred<()=>void>(),reading=deferred<AddonOverview>();let released=0;
 bridge.observe=()=>registration.promise;bridge.reading=()=>reading.promise;
 const store=createAddonStore(bridge);const release=store.subscribe(()=>{});release();registration.resolve(()=>{released++});reading.resolve(overview());await flush();
 expect(released).toBe(1);expect(store.getSnapshot().overview).toBeUndefined();
});
test("store serializes installs, reports progress, and ignores late progress",async()=>{
 const bridge=new Bridge(),download=deferred<unknown>();bridge.work=async name=>name==="download"?download.promise:undefined;
 const store=createAddonStore(bridge);const release=store.subscribe(()=>{});await flush();
 const work=store.install("example");await flush();await store.enable("other",true);
 bridge.progress!({id:"example",phase:"downloading",downloaded:20,total:100});
 expect(store.getSnapshot().progress?.downloaded).toBe(20);expect(bridge.calls.map(c=>c[0])).toEqual(["addon_install_plan","download"]);
 download.resolve(undefined);await work;
 expect(bridge.calls.map(c=>c[0])).toEqual(["addon_install_plan","download"]);
 bridge.progress!({id:"example",phase:"downloading",downloaded:99,total:100});expect(store.getSnapshot().progress).toBeUndefined();expect(store.getSnapshot().busy).toBeNull();release();
});
test("failed removal refreshes committed partial state and preserves the error",async()=>{
 const bridge=new Bridge();bridge.value.state.addons.example={enabled:true,selected:"1.0.0",versions:{"1.0.0":{manifest,consumers:[]},"2.0.0":{manifest:{...manifest,version:"2.0.0"},consumers:[]}}};
 bridge.work=async(name,args)=>{if(name==="addon_set_enabled")bridge.value={...bridge.value,state:{...bridge.value.state,addons:{example:{...bridge.value.state.addons.example,enabled:false}}}};if(name==="addon_remove_version"&&args.version==="1.0.0")throw Error("Package is pinned")};
 const store=createAddonStore(bridge),release=store.subscribe(()=>{});await flush();await store.remove("example");
 expect(bridge.calls.map(c=>c[0])).toEqual(["addon_set_enabled","addon_remove_version"]);expect(store.getSnapshot().overview?.state.addons.example.enabled).toBe(false);expect(store.getSnapshot().error).toContain("pinned");expect(store.getSnapshot().busy).toBeNull();release();
});
test("optional tabs follow active contributions; fixed tabs remain before and after onboarding",()=>{
 const tabs=["overview","nexus","frame-generation","performance","captures","options","advanced","reset"].map(id=>({id}));
 const state=overview();expect(visibleGameTabs(tabs,state).map(t=>t.id)).toEqual(["overview","performance","options","advanced","reset"]);
 const contribution:UiContribution={id:"recorder",version:"1.0.0",name:"Recorder",repository:"https://example.com",contribution:{target:"game-captures",route:"capture",title:"Capture"}};
 state.contributions=[contribution];expect(visibleGameTabs(tabs,state).map(t=>t.id)).toContain("captures");expect(gameContributions(state,"game-mods")).toEqual([]);
 state.state.selection_completed=false;expect(visibleGameTabs(tabs,state).map(t=>t.id)).toEqual(["overview","performance","captures","options","advanced","reset"]);expect(visibleGameTabs(tabs,undefined)).toBe(tabs);
});
test("delisted installed packages remain manageable without fabricated download artifacts",()=>{
 const state=overview();state.state.addons.example={enabled:false,selected:"1.0.0",versions:{"1.0.0":{manifest,consumers:[]}}};
 expect(storeEntries(state)).toEqual([{manifest,artifacts:{}}]);
 state.catalog.addons=[{manifest:{...manifest,version:"2.0.0"},artifacts:{}}];expect(storeEntries(state)).toHaveLength(1);expect(storeEntries(state)[0].manifest.version).toBe("2.0.0");
 expect(manifestCategory({...manifest,category:"captures"})).toBe("captures");expect(manifestCategory({...manifest,contributions:[{target:"game-frame-generation",route:"fg",title:"FG"}]})).toBe("frame-generation");
});

test("confirming the selection preserves categories that still have bundled integrations",()=>{
 const state=overview();
 state.bundled_targets=["game-mods","game-frame-generation"];
 const tabs=["overview","nexus","frame-generation","captures","options"].map(id=>({id}));
 expect(visibleGameTabs(tabs,state).map(tab=>tab.id)).toEqual(["overview","nexus","frame-generation","options"]);
 state.bundled_targets=[];
 expect(visibleGameTabs(tabs,state).map(tab=>tab.id)).toEqual(["overview","options"]);
});

test("downloading an add-on does not finish onboarding before explicit confirmation",async()=>{
 const bridge=new Bridge();bridge.value.state.selection_completed=false;
 const store=createAddonStore(bridge),release=store.subscribe(()=>{});await flush();
 await store.install("example");
 expect(bridge.calls.some(([name])=>name==="addon_complete_selection")).toBe(false);
 expect(store.getSnapshot().overview?.state.selection_completed).toBe(false);
 await store.complete();
 expect(bridge.calls.at(-1)?.[0]).toBe("addon_complete_selection");release();
});

test("local package inspection is read-only and import refreshes the same store",async()=>{
 const bridge=new Bridge(),store=createAddonStore(bridge),release=store.subscribe(()=>{});await flush();
 const plan={manifest:{...manifest,author:"Local author",host_version:">=0.5.5",license:"GPL-3.0-only"},sha256:"c".repeat(64),bytes:150};
 bridge.work=async(name)=>{if(name==="addon_local_plan")return plan;if(name==="addon_import_local")bridge.value.state.addons.example={enabled:true,selected:"1.0.0",versions:{"1.0.0":{manifest,consumers:[],local:true}}};};
 expect(await store.inspectLocal("/tmp/experiment.zip")).toEqual(plan);expect(store.getSnapshot().busy).toBeNull();expect(store.getSnapshot().overview?.state.addons).toEqual({});
 expect(await store.importLocal("/tmp/experiment.zip",plan.sha256)).toBe(true);
 expect(bridge.calls).toEqual([["addon_local_plan",{path:"/tmp/experiment.zip"}],["addon_import_local",{path:"/tmp/experiment.zip",sha256:plan.sha256}]]);
 expect(store.getSnapshot().overview?.state.addons.example.versions["1.0.0"].local).toBe(true);release();
});
test("failed local import keeps the current selection and reports the error",async()=>{
 const bridge=new Bridge(),store=createAddonStore(bridge),release=store.subscribe(()=>{});await flush();
 bridge.work=async()=>{throw Error("The local package changed")};
 expect(await store.importLocal("/tmp/experiment.zip","old")).toBe(false);
 expect(store.getSnapshot().error).toContain("package changed");expect(store.getSnapshot().overview?.state.addons).toEqual({});release();
});

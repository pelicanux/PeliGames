import {test,expect,mock} from "bun:test";
let cursor=0;const values:any[]=[];const dependencies:any[][]=[];let effects:(()=>unknown)[]=[];
const calls:any[]=[];
mock.module("react",()=>({useState:(initial:any)=>{const i=cursor++;if(!(i in values))values[i]=initial;return [values[i],(v:any)=>values[i]=v];},useEffect:(fn:()=>unknown,deps:any[])=>{const i=cursor++;if(!dependencies[i]||deps.some((v,j)=>v!==dependencies[i][j])){dependencies[i]=deps;effects.push(fn);}}}));
mock.module("@tauri-apps/api/core",()=>({invoke:async(command:string,args?:any)=>{calls.push({command,args});return args?.folder?`/games/${args.folder}`:"/games";}}));
const {useInstallationLocation}=await import("../src/hooks/useInstallationLocation");
const render=(name:string,open:boolean)=>{cursor=0;const hook=useInstallationLocation(name,open);const pending=effects;effects=[];pending.forEach(fn=>fn());return hook;};
test("The named default prefix is reserved after the name step and follows later edits",async()=>{
 render("",false);expect(calls).toHaveLength(0);
 render("Real MU",true);await Promise.resolve();
 let hook=render("Real MU",true);expect(hook.directory).toBe("/games/Real MU");
 await hook.selectDefault();hook=render("Real MU",true);expect(hook.selected).toBe(true);
 expect(calls.at(-1).args.folder).toBe("Real MU");
 hook=render("Real MU Teste",true);expect(hook.selected).toBe(false);
 await hook.selectDefault();hook=render("Real MU Teste",true);
 expect(await hook.resolveDirectory()).toBe("/games/Real MU Teste");
 hook.setDirectory("/custom/existing");hook=render("Outro nome",true);
 expect(await hook.resolveDirectory()).toBe("/custom/existing");
});

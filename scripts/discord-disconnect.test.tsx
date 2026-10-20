import {expect, test, mock} from "bun:test";
import type {DiscordStatus} from "../src/theme/discordBenefits";
let cursor = 0, mounted = false;
const slots: any[] = [], effects: (() => void)[] = [];
const requests: {command: string; resolve: (value: unknown) => void; reject: (error: Error) => void}[] = [];
Object.assign(globalThis,{window:new EventTarget()});
const jsx = (type: unknown, props: unknown) => ({type,props});
mock.module("react/jsx-runtime",()=>({jsx,jsxs:jsx,Fragment:"fragment"}));
mock.module("react",()=>({
 useState:(value: unknown)=>{const i=cursor++; if(!(i in slots))slots[i]=value; return [slots[i],(next:any)=>{slots[i]=typeof next==="function"?next(slots[i]):next;}];},
 useRef:(value: unknown)=>{const i=cursor++; return slots[i]??(slots[i]={current:value});},
 useEffect:(effect:()=>void)=>{if(!mounted)effects.push(effect);},
}));
mock.module("@tauri-apps/api/core",()=>({invoke:(command:string)=>new Promise((resolve,reject)=>requests.push({command,resolve,reject}))}));
mock.module("../src/i18n/I18nContext",()=>({useI18n:()=>({language:"pt"})}));
mock.module("../src/components/MenuIcon",()=>({MenuIcon:()=>null}));
mock.module("../src/services/notifications",()=>({notifyError:()=>{}}));
const {DiscordConnection}=await import("../src/components/preferences/DiscordConnection");
function render(){cursor=0; const tree=DiscordConnection(); if(!mounted){mounted=true;effects.forEach(effect=>effect());}return tree;}
function buttons(node:any):any[]{if(!node||typeof node!=="object")return [];return [...(node.type==="button"?[node]:[]),...[node.props?.children].flat(Infinity).flatMap(buttons)];}
function text(node:any):string{if(typeof node==="string")return node;if(!node||typeof node!=="object")return "";return [node.props?.children].flat(Infinity).map(text).join("");}
const flush=async()=>{for(let i=0;i<8;i++)await Promise.resolve();};
const connected:DiscordStatus={configured:true,pending:false,offline:false,account:{id:"123",name:"Teste",in_server:true,benefit:null,valid_until:null}};
test("Discord removes account immediately and ignores the pre-logout status response",async()=>{
 render(); const stale=requests.shift()!;
 window.dispatchEvent(new CustomEvent("discordConnectionChanged",{detail:connected}));
 let tree=render(); const logout=buttons(tree).find(button=>text(button)==="Desconectar");expect(logout).toBeDefined();
 logout.props.onClick();expect(requests[0].command).toBe("disconnect_discord_account");
 requests.shift()!.resolve(null);await flush();
 tree=render();expect(buttons(tree).some(button=>text(button)==="Desconectar")).toBe(false);
 expect(buttons(tree).some(button=>text(button)==="Entrar com Discord")).toBe(true);
 expect(requests).toHaveLength(0);
 stale.resolve(connected);await flush();tree=render();
 expect(buttons(tree).some(button=>text(button)==="Desconectar")).toBe(false);
});
test("Discord keeps the connected account if native credential removal fails",async()=>{
 window.dispatchEvent(new CustomEvent("discordConnectionChanged",{detail:connected}));
 let tree=render();buttons(tree).find(button=>text(button)==="Desconectar").props.onClick();
 requests.shift()!.reject(new Error("keyring failure"));await flush();tree=render();
 expect(buttons(tree).some(button=>text(button)==="Desconectar")).toBe(true);
 expect(buttons(tree).some(button=>text(button)==="Entrar com Discord")).toBe(false);
});

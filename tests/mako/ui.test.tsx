import {expect,test} from "bun:test";
import {mkdtemp,mkdir,cp,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {renderToStaticMarkup} from "react-dom/server";
import {buildAddonUi} from "../../addons/_infra/tools/build-ui";
import type {AddonHost} from "../../addons/_infra/ui-sdk";
import {importAddonModule} from "../../src/modules/addons/moduleImport";
import {ResourceScope} from "../../src/modules/addons/lifecycle";
import {createAddonI18n} from "../../src/modules/addons/i18n";
import {addonDesign} from "../../src/modules/addons/sharedDesign";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {checkMessages} from "../../scripts/i18n/catalogs";
import en from "../../addons/mako/ui/locales/en.json";
import pt from "../../addons/mako/ui/locales/pt-BR.json";
import {defaults} from "./preview";
import {connect} from "../../addons/mako/ui/src/api";
import {Controls} from "../../addons/mako/ui/src/Controls";

function host(language:string,scope:ResourceScope):AddonHost{return {...addonDesign,apiVersion:1,addon:{id:"mako",version:"0.1.0"},language,i18n:createAddonI18n(language,scope),lifecycle:scope,styles:{add(){}},service:{async call<T>(){throw new Error("SSR must not call native services");return null as T}},setBusy(){}};}
test("MAKO translations belong to its package and keep matching keys",()=>{
 expect(checkMessages({...en},{...pt},"pt-BR",false,"pt-BR")).toEqual([]);
});
test("MAKO controls preserve saved values and use the shared switch and choice design",async()=>{
 await loadLanguage("pt-BR");const scope=new ResourceScope(()=>{});const bridge=host("pt-BR",scope),api=connect(bridge);
 try {
  const settings={...defaults,enabled:true,multiplier:4,adaptive:true,allow_fp16:true,live_enabled:true,scaling_enabled:false};
  const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><Controls host={bridge} api={api} settings={settings} blocked={false} running={false} patch={()=>{}} gpus={[]}/></I18nProvider>);
  const switches=html.match(/<button[^>]*role="switch"[^>]*>/g)!;
  expect(switches).toHaveLength(6);expect(switches.every(button=>button.includes("mod-option-switch--rail"))).toBe(true);
  for(const value of ["Saída adaptativa","GPU da geração",'value="4"'])expect(html).toContain(value);
  const running=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><Controls host={bridge} api={api} settings={settings} blocked={false} running patch={()=>{}} gpus={[]}/></I18nProvider>);
  expect(running.match(/<button[^>]*role="switch"[^>]*disabled=""/g)).toHaveLength(3);

 }finally{await scope.close();}
});
test("MAKO UI builds without launcher sources and shares the host React in both languages",async()=>{
 const root=await mkdtemp(join(tmpdir(),"peligames-mako-ui-test-"));const scopes:ResourceScope[]=[];
 try {
  await mkdir(join(root,"addons/_infra"),{recursive:true});await cp("addons/mako/ui",join(root,"addons/mako/ui"),{recursive:true});await cp("addons/_infra/ui-sdk",join(root,"addons/_infra/ui-sdk"),{recursive:true});
  const source=await buildAddonUi(join(root,"addons/mako/ui/src/index.tsx"));
  for(const forbidden of ["react-dom","node_modules/react/","/home/",root])expect(source).not.toContain(forbidden);
  const module=await importAddonModule({source} as Parameters<typeof importAddonModule>[0]);
  for(const language of ["en","pt-BR"] as const){
   await loadLanguage(language);const scope=new ResourceScope(()=>{});scopes.push(scope);const bridge=host(language,scope);const styles:string[]=[];bridge.styles.add=css=>styles.push(css);
   const instance=await module.createAddon(bridge),Preferences=instance.views.preferences;
   const html=renderToStaticMarkup(<I18nProvider initialPreference={language}><Preferences context={{target:"settings",route:"preferences"}} disabled={false}/></I18nProvider>);
   expect(styles.some(css=>css.includes(".mako-controls"))).toBe(true);expect(html).toContain(language==="en"?"Prepare the component":"Prepare o componente");expect(html).toContain(language==="en"?"Download":"Baixar");expect(html).toContain("https://github.com/eugeniosegala/MAKO");
   const Game=instance.views.game;const panel=renderToStaticMarkup(<I18nProvider initialPreference={language}><Game context={{target:"game-frame-generation",route:"game",game:{id:"fixture",directory:"/fixture"}}} disabled={false}/></I18nProvider>);
   expect(panel.match(/role="tab" /g)).toHaveLength(4);expect(panel).toContain(language==="en"?"How to use":"Como usar");
   await instance.dispose();await scope.close();
  }
 }finally{await Promise.all(scopes.map(scope=>scope.close()));await rm(root,{recursive:true,force:true});}
});

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
import en from "../../addons/afmf/ui/locales/en.json";
import pt from "../../addons/afmf/ui/locales/pt-BR.json";
import {connect} from "../../addons/afmf/ui/src/api";
import {Controls} from "../../addons/afmf/ui/src/Controls";

function host(language:string,scope:ResourceScope):AddonHost{return {...addonDesign,apiVersion:1,addon:{id:"afmf",version:"0.1.0"},language,i18n:createAddonI18n(language,scope),lifecycle:scope,styles:{add(){}},service:{async call<T>(){throw new Error("SSR must not call native services");return null as T}},setBusy(){}};}
test("AFMF translations belong to its package and keep matching keys",()=>{
 expect(checkMessages({...en},{...pt},"pt-BR",false,"pt-BR")).toEqual([]);
});
test("AFMF controls preserve saved values and use the shared switch and choice design",async()=>{
 await loadLanguage("pt-BR");const scope=new ResourceScope(()=>{});const bridge=host("pt-BR",scope),api=connect(bridge);
 try {
  const settings={enabled:true,performance_mode:"quality" as const,search_mode:"high" as const,fast_motion:"repeat" as const,min_fps:45,hud_detect:true,pacing:false};
  const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><Controls host={bridge} api={api} settings={settings} blocked={false} patch={()=>{}}/></I18nProvider>);
  const switches=html.match(/<button[^>]*role="switch"[^>]*>/g)!;
  expect(switches).toHaveLength(2);expect(switches.every(button=>button.includes("mod-option-switch--rail"))).toBe(true);expect(switches.filter(button=>button.includes('aria-checked="true"'))).toHaveLength(1);
  for(const value of ["Qualidade","Alta","Repetir",'value="45"'])expect(html).toContain(value);
 }finally{await scope.close();}
});
test("AFMF UI builds without launcher sources and shares the host React in both languages",async()=>{
 const root=await mkdtemp(join(tmpdir(),"peligames-afmf-ui-test-"));const scopes:ResourceScope[]=[];
 try {
  await mkdir(join(root,"addons/_infra"),{recursive:true});await cp("addons/afmf/ui",join(root,"addons/afmf/ui"),{recursive:true});await cp("addons/_infra/ui-sdk",join(root,"addons/_infra/ui-sdk"),{recursive:true});
  const source=await buildAddonUi(join(root,"addons/afmf/ui/src/index.tsx"));
  for(const forbidden of ["react-dom","node_modules/react/","/home/",root])expect(source).not.toContain(forbidden);
  const module=await importAddonModule({source} as Parameters<typeof importAddonModule>[0]);
  for(const language of ["en","pt-BR"] as const){
   await loadLanguage(language);const scope=new ResourceScope(()=>{});scopes.push(scope);const bridge=host(language,scope);const styles:string[]=[];bridge.styles.add=css=>styles.push(css);
   const instance=await module.createAddon(bridge),Preferences=instance.views.preferences;
   const html=renderToStaticMarkup(<I18nProvider initialPreference={language}><Preferences context={{target:"settings",route:"preferences"}} disabled={false}/></I18nProvider>);
   expect(styles.some(css=>css.includes(".afmf-controls"))).toBe(true);expect(html).toContain(language==="en"?"AFMF component":"Componente AFMF");expect(html).toContain(language==="en"?"Download":"Baixar");expect(html).toContain("https://github.com/serialexperimentslainnnn/afmf-linux");
   const Game=instance.views.game;const panel=renderToStaticMarkup(<I18nProvider initialPreference={language}><Game context={{target:"game-frame-generation",route:"game",game:{id:"fixture",directory:"/fixture"}}} disabled={false}/></I18nProvider>);
   expect(panel.match(/role="tab" /g)).toHaveLength(4);expect(panel).toContain(language==="en"?"How to use":"Como usar");
   await instance.dispose();await scope.close();
  }
 }finally{await Promise.all(scopes.map(scope=>scope.close()));await rm(root,{recursive:true,force:true});}
});

import {expect,test} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {buildAddonUi} from "../../addons/_infra/tools/build-ui";
import {importAddonModule} from "../../src/modules/addons/moduleImport";
import {ResourceScope} from "../../src/modules/addons/lifecycle";
import {createAddonI18n} from "../../src/modules/addons/i18n";
import {addonDesign} from "../../src/modules/addons/sharedDesign";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
Object.assign(globalThis,{__APP_VERSION__:"0.5.5",__BUILD_SUFFIX__:"test"});
const {GameModsTabs}=await import("../../src/modules/nexus/GameModsTabs");
import {createMochiPreview} from "./preview";
import en from "../../addons/dlssnr-mochizuki/ui/locales/en.json";
import pt from "../../addons/dlssnr-mochizuki/ui/locales/pt-BR.json";

test("Mochizuki builds as an independent translated interface without starting a service",async()=>{
 expect(Object.keys(pt).sort()).toEqual(Object.keys(en).sort());
 const source=await buildAddonUi("addons/dlssnr-mochizuki/ui/src/index.tsx");
 expect(source).not.toContain("@tauri-apps");expect(source).not.toContain("/src-tauri/");expect(source).not.toContain("mauri870");
 const module=await importAddonModule({source} as Parameters<typeof importAddonModule>[0]);
 for(const language of ["en","pt-BR"] as const){
  await loadLanguage(language);const scope=new ResourceScope(()=>{}),calls:string[]=[];
  const host={...addonDesign,apiVersion:1 as const,addon:{id:"dlssnr-mochizuki",version:"0.1.0"},language,i18n:createAddonI18n(language,scope),lifecycle:scope,styles:{add(){}},service:{async call<T>(method:string){calls.push(method);return null as T}},setBusy(){}};
  const instance=await module.createAddon(host);
  const View=instance.views.preferences;
  const html=renderToStaticMarkup(<I18nProvider initialPreference={language}><View context={{target:"settings",route:"preferences"}} disabled={false}/></I18nProvider>);
  expect(html).toContain(language==="en"?"Download":"Baixar");expect(html).toContain("https://gitlab.com/mochizuki0323/DLSSNR-AMD");
  const Game=instance.views.game;
  const gameHtml=renderToStaticMarkup(<I18nProvider initialPreference={language}><Game context={{target:"game-mods",route:"game",game:{id:"fixture",directory:"/fixture",executableDirectory:"/fixture/game"}}} disabled={false}/></I18nProvider>);
  expect(gameHtml).toContain(language==="en"?"Starting network":"Rede inicial");
  expect(gameHtml).toContain("selection-menu-compact");expect(calls).toEqual([]);
  await instance.dispose();await scope.close();
 }
});
test("NR backends appear only from installed contributions, without the retired Mauri card",()=>{
 const manifest=createMochiPreview().manifest;
 const entry={id:manifest.id,version:manifest.version,name:manifest.name,repository:manifest.repository,group:manifest.group,contribution:manifest.contributions.find(c=>c.target==="game-mods")!};
 const render=(entries:typeof entry[])=>renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><GameModsTabs activeTab="dlss" onTabChange={()=>{}} nexus={null} externalEntries={entries}/></I18nProvider>);
 expect(render([])).not.toContain("Mochizuki");expect(render([entry])).toContain("Mochizuki");expect(render([entry])).not.toContain("Mauri");
});
test("preview component removal preserves mod copies inside games",()=>{
 const preview=createMochiPreview();preview.call("runtime.remove",{});
 expect(preview.call("runtime.inspect",{})).toMatchObject({ready:false,models:false});
 expect(preview.call("game.inspect",{directory:"/fixture/game"})).toMatchObject({installation:{dlssnr_installed:true}});
});

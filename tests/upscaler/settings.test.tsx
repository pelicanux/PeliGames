import {OptiScalerConnection} from "./host";
import {test,expect} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {GameModsTabs,type GameModsTab} from "../../src/modules/nexus/GameModsTabs";
import {AddonCatalog} from "../../src/components/preferences/AddonCatalog";
import {UpscalerControls} from "../../addons/optiscaler/ui/src/UpscalerPanel";
import {createUpscalerChoice} from "../../addons/optiscaler/ui/src/choice";
import {defaultUpscalerSettings,upscalerFields,upscalerDisplayValue,upscalerValueLabel,upscalerToggle,upscalerFieldValues} from "../../addons/optiscaler/ui/src/settings";
import {OptiScalerPreferences} from "../../addons/optiscaler/ui/src/OptiScalerPreferences";
import {OptiScalerUsage} from "../../addons/optiscaler/ui/src/OptiScalerUsage";
await loadLanguage("pt-BR");
const flush=async()=>{for(let i=0;i<20;i++)await Promise.resolve();};
function mods(tab:GameModsTab,disabled=false){return renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><GameModsTabs activeTab={tab} externalEntries={[
 {id:"fixture",version:"1.0.0",name:"OptiScaler",repository:"https://example.com",contribution:{target:"game-mods",route:"game",title:"Fixture"}},
 {id:"dlssnr-mochizuki",version:"0.1.0",name:"RX 9000 · Mochizuki",repository:"https://gitlab.com/mochizuki0323/DLSSNR-AMD",group:"dlssnr",contribution:{target:"game-mods",route:"game",title:"Neural Rendering"}},
 ]} onTabChange={()=>{}} disabled={disabled} nexus={<div>NEXUS_BODY</div>}/></OptiScalerConnection></I18nProvider>);}
test("the Mods catalog uses installed backend contributions without retired or experimental projects",()=>{
 const root=mods("catalog");for(const name of ["OptiScaler","Nexus Mods","DLSSNR"])expect(root).toContain(`aria-label="${name}"`);
 expect(root).toContain("component-catalog-button");expect(root).not.toContain('role="tablist"');expect(root).not.toContain('Daniel');
 const backends=mods("dlss");expect(backends).toContain('RX 9000 · Mochizuki');expect(backends).not.toContain('Mauri');expect(backends).not.toContain('Daniel');expect(backends).not.toContain('NEXUS_BODY');
 expect(mods("nexus")).toContain('NEXUS_BODY');expect(mods("catalog",true)).toContain('disabled=""');
 const addon=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><AddonCatalog onOpen={()=>{}} disabled={false}/></OptiScalerConnection></I18nProvider>);expect(addon).toContain('component-catalog-button');
});
test("upscaler fields retain custom installed values and expose a distinct frame generation card",()=>{
 const settings={...defaultUpscalerSettings(),"Upscalers.Dx12Upscaler":"custom-network","Menu.Scale":"1.25","Menu.ShortcutKey":"0x22"};
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><UpscalerControls choice={createUpscalerChoice(settings)} installed blocked={false}/></OptiScalerConnection></I18nProvider>);
 for(const expected of ["Personalizado: custom-network","125%","Page Down","Latência","Auto","Geração de quadros","Entrada do FG","Saída do FG","Provedor DLSSG / NGX"])expect(html).toContain(expected);
 expect(html).not.toContain('FrameGen.Enabled');expect(html).not.toContain('NeuralUplift');
});
test("changing different fields quickly serializes every changed field and restores actual partial confirmations after failure",async()=>{
 const initial={a:"auto",b:"auto",custom:"keep"};const writes:unknown[]=[];let release!:()=>void;let blocked=true,fail=false;
 const choice=createUpscalerChoice(initial,async(key,value)=>{writes.push([key,value]);if(blocked){blocked=false;await new Promise<void>(r=>release=r);}if(fail&&key==="b")throw Error("denied");});
 choice.change("a","one");choice.change("b","two");choice.change("a","three");expect(choice.getSnapshot().values).toMatchObject({a:"three",b:"two"});release();await flush();
 expect(writes).toEqual([["a","one"],["a","three"],["b","two"]]);expect(choice.getSnapshot().values).toEqual({a:"three",b:"two",custom:"keep"});
 fail=true;choice.change("a","four");choice.change("b","five");await flush();expect(choice.getSnapshot().values).toEqual({a:"four",b:"two",custom:"keep"});expect(choice.getSnapshot().error).toContain("denied");
});
test("pre-install drafts do not write native files and stores stay isolated by game",async()=>{
 const a=createUpscalerChoice(defaultUpscalerSettings());const b=createUpscalerChoice(defaultUpscalerSettings());a.change("Menu.Scale","1.5");await flush();expect(a.getSnapshot().values["Menu.Scale"]).toBe("1.5");expect(b.getSnapshot().values["Menu.Scale"]).toBe("auto");expect(a.getSnapshot().saved).toBe(false);
});

test("frame generation saves the native keys and providers independently from upscaling and NR",async()=>{
 const writes:unknown[]=[];const c=createUpscalerChoice({...defaultUpscalerSettings(),"FrameGen.FGNvngxReplacement":"None"},async(key,value)=>{writes.push([key,value])});
 c.change("FrameGen.FGInput","nvngxfg");c.change("FrameGen.FGNvngxReplacement","Nukems");c.change("FrameGen.FGOutput","fsrfg");c.change("FrameGen.Enabled","true");await flush();
 expect(c.getSnapshot().values).toMatchObject({"FrameGen.FGInput":"nvngxfg","FrameGen.FGNvngxReplacement":"Nukems","FrameGen.FGOutput":"fsrfg","FrameGen.Enabled":"true","Upscalers.Dx12Upscaler":"auto"});
 expect(writes).toContainEqual(["FrameGen.FGInput","nvngxfg"]);expect(writes).toContainEqual(["FrameGen.FGOutput","fsrfg"]);expect(writes).toContainEqual(["FrameGen.FGNvngxReplacement","Nukems"]);expect(writes).toContainEqual(["FrameGen.Enabled","true"]);
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><UpscalerControls choice={c} installed blocked={false}/></OptiScalerConnection></I18nProvider>);expect(html).toContain("Nukem · FSR 3 FG");expect(html).toContain("FSR FG");expect(html).toContain("DLSSG via NGX");
 const old=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><UpscalerControls choice={c} installed blocked={false} framegenAvailable={false}/></OptiScalerConnection></I18nProvider>);expect(old).toContain("Atualize o componente");expect(old).toMatch(/disabled=""[^>]*aria-label="Entrada do FG"|aria-label="Entrada do FG"[^>]*disabled=""/);
});

test("frame generation has explicit manual choices and new drafts start disabled",()=>{
 const defaults=defaultUpscalerSettings();expect(defaults).toMatchObject({"FrameGen.Enabled":"false","FrameGen.FGInput":"nofg","FrameGen.FGOutput":"nofg","FrameGen.FGNvngxReplacement":"Nukems"});
 for(const field of upscalerFields.filter(f=>f.group==="framegeneration"))expect([...field.values]).not.toContain("auto");
 expect(upscalerDisplayValue("FrameGen.FGInput","auto")).toBe("auto");expect(upscalerDisplayValue("FrameGen.FGOutput","auto")).toBe("auto");expect(upscalerDisplayValue("FrameGen.Enabled","auto")).toBe("auto");
 expect(upscalerValueLabel("Upscalers.Dx12Upscaler","ffx","Padrão","Ligado","Desligado")).toBe("FSR");
 const choice=createUpscalerChoice({...defaults,"FrameGen.Enabled":"auto","FrameGen.FGInput":"dlssg","FrameGen.FGNvngxReplacement":"auto"});
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><UpscalerControls choice={choice} installed blocked={false}/></OptiScalerConnection></I18nProvider>);
 expect(html).toMatch(/role="switch" aria-checked="false" aria-label="Ativação do FG"/);expect(html).toContain("Auto");expect(html).not.toContain("FidelityFX");expect(html).not.toContain("Automático");
 expect(choice.getSnapshot().values["FrameGen.Enabled"]).toBe("auto");
});

test("binary OptiScaler controls use accessible switches without replacing inherited defaults",async()=>{
 expect(upscalerToggle("FrameGen.Enabled","false")).toEqual({enabled:false,inherited:false,next:"true"});
 expect(upscalerToggle("fakenvapi.ForceReflex","auto")).toEqual({enabled:false,inherited:true,next:"2"});
 expect(upscalerToggle("fakenvapi.ForceReflex","2")).toEqual({enabled:true,inherited:false,next:"1"});
 expect(upscalerToggle("Upscalers.Dx12Upscaler","auto")).toBeNull();
 const writes:unknown[]=[];const choice=createUpscalerChoice(defaultUpscalerSettings(),async(key,value)=>{writes.push([key,value])});
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><UpscalerControls choice={choice} installed blocked={false}/></OptiScalerConnection></I18nProvider>);
 const switches=html.match(/<button[^>]*role="switch"[^>]*>/g)??[];
 for(const label of ["Ativação do FG","Capturar chamadas DLSS","Spoof de GPU para DLSS (DXGI)","Forçar estado do Reflex","Forçar LatencyFlex"])expect(switches.some(button=>button.includes(`aria-label="${label}"`))).toBe(true);
 expect(html).toContain('aria-label="Entrada do FG"');expect(writes).toEqual([]);
 choice.change("fakenvapi.ForceReflex",upscalerToggle("fakenvapi.ForceReflex","auto")!.next);await flush();expect(writes).toEqual([["fakenvapi.ForceReflex","2"]]);expect(choice.getSnapshot().values["Spoofing.Dxgi"]).toBe("auto");
});

test("management remains available before installation while every INI editor is locked",()=>{
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><UpscalerControls choice={createUpscalerChoice({})} installed={false} blocked configurationAvailable={false} management={<div>GAME_INSTALLATION</div>}/></OptiScalerConnection></I18nProvider>);
 expect(html).toMatch(/role="tab"[^>]*aria-selected="true"[^>]*>.*?Gerenciar/);
 expect(html).toContain("GAME_INSTALLATION");
 const tabs=html.match(/<button[^>]*role="tab"[^>]*>/g)!;
 expect(tabs).toHaveLength(5);
 expect(tabs.filter(tab=>tab.includes('disabled=""'))).toHaveLength(4);
 expect(html).toMatch(/disabled=""[^>]*aria-label="Escala do menu"/);
 expect(html).not.toContain("Padrão do OptiScaler");
 const embedded=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><OptiScalerPreferences embedded disabled={false} onBusy={()=>{}}/></OptiScalerConnection></I18nProvider>);
 expect(embedded).toContain('role="switch" aria-checked="false" aria-label="Versão de testes"');
});

test("the Enabler providers are not offered and INI auto values are not rewritten as disabled",()=>{
 const providers=upscalerFields.find(field=>field.key==="FrameGen.FGNvngxReplacement")!.values;
 expect([...providers]).toEqual(["None","Nukems","FFX"]);
 const values={...defaultUpscalerSettings(),"FrameGen.FGInput":"auto","FrameGen.FGOutput":"auto","FrameGen.FGNvngxReplacement":"Arturs"};
 const choice=createUpscalerChoice(values);
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><UpscalerControls choice={choice} installed blocked={false}/></OptiScalerConnection></I18nProvider>);
 expect(html).not.toContain("DLSS Enabler");
 expect(html).toContain("Personalizado: Arturs");
 expect(choice.getSnapshot().values).toEqual(values);
});

test("How to use is available before installation and reuses the PeliRun exporter after installation",async()=>{
 const game={name:"Test game",path:"/games/test",launcher:"Steam"};
 for(const language of ["pt-BR","en"] as const){
  await loadLanguage(language);
  const title=language==="pt-BR"?"Como usar":"How to use";
  const action=language==="pt-BR"?"Preparar comando PeliRun":"Prepare PeliRun command";
  const usage=<OptiScalerUsage game={game} installed={false} blocked={false}/>;
  const html=renderToStaticMarkup(<I18nProvider initialPreference={language}><OptiScalerConnection game={game}><UpscalerControls choice={createUpscalerChoice({})} installed={false} blocked configurationAvailable={false} management={<div>MANAGEMENT</div>} usage={usage}/></OptiScalerConnection></I18nProvider>);
  expect(html).toContain(`<span>${title}</span>`);
  const tabs=html.match(/<button[^>]*role="tab"[^>]*>/g)!;
  expect(tabs).toHaveLength(6);
  expect(tabs.at(-1)).not.toContain('disabled=""');
  expect(html).toContain(`disabled="">${action}`);
  expect(html).toContain("PeliRun");expect(html).not.toContain("bgmod");
  const ready=renderToStaticMarkup(<I18nProvider initialPreference={language}><OptiScalerConnection game={game}><OptiScalerUsage game={game} installed blocked={false}/></OptiScalerConnection></I18nProvider>);
  expect(ready).toContain(`class="modern-mini-btn">${action}</button>`);
  const blocked=renderToStaticMarkup(<I18nProvider initialPreference={language}><OptiScalerConnection game={game}><OptiScalerUsage game={game} installed blocked/></OptiScalerConnection></I18nProvider>);
  expect(blocked).toContain(`disabled="">${action}`);
 }
});

test("FSR backend is not mislabeled as a fixed library version and compatibility follows the installed INI",()=>{
 const modern=defaultUpscalerSettings();const stable=defaultUpscalerSettings(false);
 expect(upscalerValueLabel("FSR.UpscalerIndex","0","Auto","On","Off")).toBe("FSR 4");
 expect(upscalerFieldValues(upscalerFields.find(f=>f.key==="Upscalers.Dx12Upscaler")!,stable)).toContain("fsr31");
 expect(upscalerFieldValues(upscalerFields.find(f=>f.key==="Upscalers.Dx12Upscaler")!,stable)).not.toContain("ffx");
 expect(upscalerFieldValues(upscalerFields.find(f=>f.key==="Upscalers.Dx12Upscaler")!,modern)).toContain("ffx");
 const render=(values:Record<string,string>)=>renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><UpscalerControls choice={createUpscalerChoice(values)} installed blocked={false}/></OptiScalerConnection></I18nProvider>);
 const oldHtml=render(stable);const newHtml=render(modern);const unavailable=render({});
 expect(oldHtml).toContain('aria-label="Forçar FSR 4 INT8"');expect(oldHtml).not.toContain('aria-label="Modelo do FSR 4"');
 expect(newHtml).toContain('aria-label="Forçar suporte da GPU ao FSR 4"');expect(newHtml).toContain('aria-label="Modelo do FSR 4"');expect(newHtml).not.toContain('aria-label="Forçar FSR 4 INT8"');
 expect(unavailable).not.toContain('aria-label="Forçar FSR 4 INT8"');expect(unavailable).not.toContain('aria-label="Modelo do FSR 4"');
 expect(newHtml).not.toContain('FSR 4.0.2');expect(newHtml).toContain('4.1.2');
 expect(upscalerToggle("FSR.Fsr4ForceEnableInt8","auto")).toEqual({enabled:false,inherited:true,next:"true"});
 expect(upscalerToggle("FSR.Fsr4ForceEnableInt8","true")).toEqual({enabled:true,inherited:false,next:"false"});
});

test("the launcher does not offer an embedded OptiScaler when no external contribution is installed",()=>{
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><GameModsTabs activeTab="catalog" onTabChange={()=>{}} nexus={null} dlss={null}/></I18nProvider>);
 expect(html).not.toContain("OptiScaler");expect(html).toContain("Nexus Mods");
});

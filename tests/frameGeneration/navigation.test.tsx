import {test,expect} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {SettingsCategories} from "../../src/components/SettingsCategories";
import {ModDetailHeader} from "../../src/components/ModDetailHeader";
import {featureReportContext} from "../../src/modules/reports/reportContext";
import afmfManifest from "../../addons/afmf/manifest.json";
const game={name:"Test game",path:"/preview/game",launcher:"PeliGames"};

test("shared settings panels link every tab to its content and keep unavailable tabs out of keyboard order",()=>{
 const html=renderToStaticMarkup(<SettingsCategories label="Components" active="prepare" onChange={()=>{}} categories={[{id:"manage",label:"Manage",icon:"download",content:<div>KEEP_MANAGER</div>},{id:"prepare",label:"Setup",icon:"settings",content:<input defaultValue="KEEP_DRAFT"/>},{id:"unavailable",label:"Unavailable",icon:"tools",disabled:true,content:<div>DISABLED_CONTENT</div>}]}/>);
 const ids=[...html.matchAll(/aria-controls="([^"]+)"/g)].map(match=>match[1]);
 expect(ids).toHaveLength(3);for(const id of ids)expect(html).toContain(`id="${id}"`);
 expect(html).toContain('value="KEEP_DRAFT"');expect(html).toContain('KEEP_MANAGER');expect(html).toContain('tabindex="-1" disabled=""');
});

test("the report action shares the detail header without a second toolbar and retains the original report context",async()=>{
 const globals=globalThis as Record<string,unknown>;
 const previousVersion=globals.__APP_VERSION__,previousSuffix=globals.__BUILD_SUFFIX__;
 globals.__APP_VERSION__="0.0.0";globals.__BUILD_SUFFIX__="test";
 const {FeatureReportSection}=await import("../../src/modules/reports/FeatureReportSection");
 if(previousVersion===undefined)delete globals.__APP_VERSION__;else globals.__APP_VERSION__=previousVersion;
 if(previousSuffix===undefined)delete globals.__BUILD_SUFFIX__;else globals.__BUILD_SUFFIX__=previousSuffix;
 await loadLanguage("pt-BR");
 const context=featureReportContext(game,"frame-generation",game.path);
 const inline=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><FeatureReportSection context={context} label="Geração de quadros">{action=><><ModDetailHeader title="AFMF" project={{name:"AFMF",url:afmfManifest.repository}} backLabel="Todos os geradores" onBack={()=>{}} actions={action}/><div>SETTINGS_BODY</div></>}</FeatureReportSection></I18nProvider>);
 expect(inline.match(/class="modern-mini-btn feature-report-button"/g)).toHaveLength(1);
 expect(inline).not.toContain('class="feature-report-toolbar"');
 const header=inline.slice(inline.indexOf('<header'),inline.indexOf('</header>'));
 for(const text of ['<h3>AFMF</h3>',"Todos os geradores","Reportar erro","Conheça o projeto",afmfManifest.repository])expect(header).toContain(text);
 expect(inline).toContain('rel="noopener noreferrer"');expect(inline).toContain("SETTINGS_BODY");
 const original=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><FeatureReportSection context={context} label="Original"><div>OTHER_FEATURE</div></FeatureReportSection></I18nProvider>);
 expect(original).toContain('class="feature-report-toolbar"');expect(original).toContain("OTHER_FEATURE");
});

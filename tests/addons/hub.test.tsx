import {expect,test} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {AddonStoreCard} from "../../src/modules/addons/AddonStoreCard";
import {HubBanners} from "../../src/modules/addons/HubBanners";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import type {AddonOverview} from "../../src/modules/addons/contracts";
import type {AddonStoreController} from "../../src/modules/addons/store";
const manifest={id:"example",name:"Example",version:"1.0.0",summary:"Example",repository:"https://example.org",contributions:[],dependencies:{}};
const overview:AddonOverview={catalog:{addons:[]},state:{selection_completed:true,addons:{example:{selected:"1.0.0",enabled:true,versions:{"1.0.0":{manifest,consumers:[]}}}}},platform:"linux-x86_64",host_version:"0.5.5",contributions:[]};
test("Hub activation is limited to installed view and installation uses a circular progress icon",async()=>{
 await loadLanguage("pt-BR");
 const card=(installedView:boolean,installed=true)=>renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><AddonStoreCard entry={{manifest,artifacts:{"linux-x86_64":{bytes:100}}}} overview={installed?overview:{...overview,state:{selection_completed:true,addons:{}}}} snapshot={{overview,loading:false,error:"",busy:installed?null:"example",progress:installed?undefined:{id:"example",phase:"verifying",downloaded:100,total:100}}} controller={{} as AddonStoreController} blocked={false} onOpen={()=>{}} installedView={installedView}/></I18nProvider>);
 expect(card(false)).not.toContain('role="switch"');expect(card(true)).toContain('role="switch"');
 const installing=card(false,false);expect(installing).toContain('addon-install-ring');expect(installing).toContain('aria-busy="true"');expect(installing).toContain('data-icon="settings"');expect(installing).not.toContain('Add-on não oficial');
});
test("community banners use local artwork, translated text and the actual Discord destination",async()=>{
 await loadLanguage("pt-BR");const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><HubBanners/></I18nProvider>);
 expect(html).toContain('/addons/banners/discord.webp');expect(html).toContain('src="/branding/peligames-glass.webp"');expect(html).toContain('https://discord.gg/78XSB9bHst');expect(html).toContain('Entrar no Discord');expect(html.match(/aria-pressed=/g)).toHaveLength(4);
});

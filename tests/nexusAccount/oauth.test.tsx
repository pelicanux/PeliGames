import {test,expect} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {NexusAccountPanel} from "../../src/modules/nexus/NexusAccountPanel";

test("Nexus offers browser authorization and no personal API key form in either language",async()=>{
  const previous=Object.getOwnPropertyDescriptor(globalThis,"window");
  Object.defineProperty(globalThis,"window",{configurable:true,value:{}});
  try { for(const language of ["pt-BR","en"] as const){
    await loadLanguage(language);
    const html=renderToStaticMarkup(<I18nProvider initialPreference={language}><NexusAccountPanel/></I18nProvider>);
    expect(html).toContain(language==="pt-BR"?"Entrar com Nexus Mods":"Sign in with Nexus Mods");
    expect(html).not.toContain('type="password"');
    expect(html).not.toContain('<form');
    expect(html).not.toContain('tab=api');
    expect(html).not.toContain(language==="pt-BR"?"chave pessoal":"personal key");
    expect(html).toContain(language==="pt-BR"?"Mods locais":"Local mods");
  }} finally {
    if(previous)Object.defineProperty(globalThis,"window",previous);
    else Reflect.deleteProperty(globalThis,"window");
  }
});

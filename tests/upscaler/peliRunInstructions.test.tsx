import {test,expect} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {PeliRunInstructions,type PeliRunTarget} from "../../src/components/PeliRunInstructions";

const profile={id:"test-game",executable:"/games/tools/pelirun",arguments:"--profile test-game --",line:"'/games/tools/pelirun' --profile test-game --"};
test("prepared PeliRun commands use the chosen launcher's syntax and keep advanced help collapsed",async()=>{
 for(const language of ["pt-BR","en"] as const){
  await loadLanguage(language);
  const render=(target:PeliRunTarget,copied="")=>renderToStaticMarkup(<I18nProvider initialPreference={language}><PeliRunInstructions profile={profile} initialTarget={target} copied={copied} onCopy={()=>{}} onRefresh={()=>{}} disabled={false}/></I18nProvider>);
  const steam=render("Steam");expect(steam).toContain("%command%");expect(steam).toContain('aria-pressed="true">Steam');expect(steam).toContain('<details class="pelirun-export-help">');expect(steam).not.toContain('<details open');
  for(const target of ["Heroic","Lutris","Outro"] as const)expect(render(target)).not.toContain("%command%");
  const heroic=render("Heroic");expect(heroic.match(/<input /g)).toHaveLength(2);expect(heroic).toContain('value="/games/tools/pelirun"');expect(heroic).toContain('value="--profile test-game --"');
  expect(render("Lutris","Steam:line")).not.toContain(language==="pt-BR"?">Copiado<":">Copied<");
  expect(render("Steam","Steam:line")).toContain(language==="pt-BR"?">Copiado<":">Copied<");
 }
});

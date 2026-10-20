import {test,expect,mock} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
mock.module("../src/i18n/I18nContext",()=>({useI18n:()=>({language:"pt",t:(_section:string,key:string)=>key==="defaultKey"?"Padrão":key})}));
const {ShortcutKeySelector}=await import("../src/components/ShortcutKeySelector");
const render=(route:string,key:string,disabled=false)=>renderToStaticMarkup(<ShortcutKeySelector route={route} shortcutKey={key} disabled={disabled} setShortcutKey={()=>{}}/>);
test("Default label follows the route, not the last key chosen",()=>{
 expect(render("reshade","Home")).toContain("Padrão");
 expect(render("optiscaler","Insert")).toContain("Padrão");
 for(const route of ["reshade","vulkan","dx9","optiscaler"]){
  const custom=render(route,"F12");expect(custom).toContain("F12");expect(custom).not.toContain("Padrão");
 }
 expect(render("reshade","Home")).toContain("Padrão");
});
test("Disabled state and the reserved End warning remain in place",()=>{
 expect(render("reshade","F12",true)).toContain('disabled=""');
 expect(render("reshade","End")).toContain("End não está disponível");
});

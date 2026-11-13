import {OptiScalerConnection} from "./host";
import {test,expect} from "bun:test";
import {renderToStaticMarkup} from "react-dom/server";
import {I18nProvider} from "../../src/i18n/I18nContext";
import {loadLanguage} from "../../src/i18n/engine";
import {OptiScalerMenuKey} from "../../addons/optiscaler/ui/src/OptiScalerMenuKey";
import {optiscalerMenuKey,optiscalerMenuKeyLabel} from "../../addons/optiscaler/ui/src/menuShortcut";
import {defaultUpscalerSettings} from "../../addons/optiscaler/ui/src/settings";
await loadLanguage("pt-BR");
const event=(code:string)=>({code,ctrlKey:false,altKey:false,shiftKey:false,metaKey:false});
test("menu recorder maps real keyboard codes to Windows keys and preserves legacy values",()=>{
 for(const [code,value,label] of [["Insert","0x2D","Insert"],["PageDown","0x22","Page Down"],["F10","0x79","F10"],["F24","0x87","F24"],["KeyP","0x50","P"],["Digit7","0x37","7"],["Numpad2","0x62","Numpad 2"],["Backspace","0x08","Backspace"]]){
  expect(optiscalerMenuKey(event(code))).toBe(value);expect(optiscalerMenuKeyLabel(value)).toBe(label);
 }
 expect(optiscalerMenuKeyLabel("auto")).toBe("Insert");expect(optiscalerMenuKeyLabel("121")).toBe("F10");
 for(const modifier of ["ctrlKey","altKey","shiftKey","metaKey"])expect(optiscalerMenuKey({...event("F10"),[modifier]:true})).toBeNull();
 expect(optiscalerMenuKey(event("Escape"))).toBeNull();expect(optiscalerMenuKey(event("Unidentified"))).toBeNull();
});
test("new installations show Insert and an editable recorder instead of a menu-key dropdown",()=>{
 expect(defaultUpscalerSettings()["Menu.ShortcutKey"]).toBe("0x2D");
 const html=renderToStaticMarkup(<I18nProvider initialPreference="pt-BR"><OptiScalerConnection><OptiScalerMenuKey value="auto" disabled={false} onChange={()=>{}}/></OptiScalerConnection></I18nProvider>);
 expect(html).toContain("Insert");expect(html).toContain("Padrão");expect(html).toContain("Clique para gravar outro atalho");expect(html).toContain("Restaurar Insert");expect(html).not.toContain('role="listbox"');
});

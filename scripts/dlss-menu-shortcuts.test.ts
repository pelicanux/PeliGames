import {test, expect} from "bun:test";
import {menuShortcutForRoute, menuShortcutStorageKey} from "../src/services/routeDefaults";
test("OptiScaler e ReShade mantêm teclas independentes no mesmo jogo",()=>{
 const game="/games/example", keys:Record<string,string>={};
 expect(menuShortcutForRoute(keys,game,game,"optiscaler")).toBe("Insert");
 expect(menuShortcutForRoute(keys,game,game,"reshade")).toBe("Home");
 keys[menuShortcutStorageKey(game,"optiscaler")]="F1";
 expect(menuShortcutForRoute(keys,game,game,"reshade")).toBe("Home");
 keys[menuShortcutStorageKey(game,"reshade")]="F12";
 for(const route of ["reshade","vulkan","dx9"]) expect(menuShortcutForRoute(keys,game,game,route)).toBe("F12");
 expect(menuShortcutForRoute(keys,game,game,"optiscaler")).toBe("F1");
});
test("Preferências antigas continuam disponíveis sem sobrescrever o outro menu",()=>{
 const keys:Record<string,string>={"/games/example":"Delete"};
 keys[menuShortcutStorageKey("steam:123","optiscaler")]="F2";
 expect(menuShortcutForRoute(keys,"steam:123","/games/example","optiscaler")).toBe("F2");
 expect(menuShortcutForRoute(keys,"steam:123","/games/example","reshade")).toBe("Delete");
});

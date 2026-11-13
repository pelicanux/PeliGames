import {expect, test} from "bun:test";
import {createAddonI18n} from "../../src/modules/addons/i18n";
import {ResourceScope} from "../../src/modules/addons/lifecycle";
import {engine, loadLanguage, translator} from "../../src/i18n/engine";

test("add-ons share the launcher's language rules with scoped messages, plurals and fallback", async () => {
  await loadLanguage("pt-BR");
  const scope = new ResourceScope();
  const i18n = createAddonI18n("pt-BR", scope);
  const t = i18n.createTranslator({defaultLanguage:"en", resources:{
    en: {fallback:"English fallback", installed:"Add-on text", count_one:"{{count}} file", count_other:"{{count}} files"},
    pt: {installed:"Texto do add-on", count_one:"{{count}} arquivo", count_other:"{{count}} arquivos", hello:"Olá, {{name}}"},
  }});
  expect(i18n.language).toBe("pt-BR");
  expect(t("installed")).toBe("Texto do add-on");
  expect(t("count", {count:1})).toBe("1 arquivo");
  expect(t("count", {count:3})).toBe("3 arquivos");
  expect(t("hello", {name:"Peli", lng:"en", ns:"addons", fallbackLng:"fr"})).toBe("Olá, Peli");
  expect(t("fallback")).toBe("English fallback");
  expect(translator("pt-BR")("addons","installed")).toBe("Complementos instalados");
  expect(engine.hasResourceBundle("pt", "addon")).toBe(false);
  await scope.close();
  expect(()=>t("installed")).toThrow("closed");
  expect(()=>i18n.createTranslator({defaultLanguage:"en",resources:{en:{}}})).toThrow("closed");
});

test("add-on catalogs are isolated, copied and released separately", async () => {
  const first = new ResourceScope(), second = new ResourceScope();
  const resources = {en:{name:"First module"}};
  const t1 = createAddonI18n("de", first).createTranslator({defaultLanguage:"en",resources});
  const t2 = createAddonI18n("pt-BR", second).createTranslator({defaultLanguage:"es",resources:{es:{name:"Segundo módulo"}}});
  resources.en.name="Changed after registration";
  expect(t1("name")).toBe("First module");
  expect(t2("name")).toBe("Segundo módulo");
  await first.close();
  expect(t2("name")).toBe("Segundo módulo");
  await second.close();
});

test("invalid or oversized translation catalogs fail inside the add-on", async () => {
  const scope=new ResourceScope();
  const i18n=createAddonI18n("en",scope);
  expect(()=>i18n.createTranslator({defaultLanguage:"en",resources:{pt:{name:"nome"}}})).toThrow("default language");
  expect(()=>i18n.createTranslator({defaultLanguage:"en",resources:{en:{huge:"x".repeat(1024*1024+1)}}})).toThrow("oversized");
  expect(()=>i18n.createTranslator({defaultLanguage:"pt-BR",resources:{"pt-br":{name:"um"},"pt-BR":{name:"dois"}}})).toThrow("Invalid");
  expect(i18n.createTranslator({defaultLanguage:"en",resources:{en:{name:"Valid"}}})("name")).toBe("Valid");
  await scope.close();
});

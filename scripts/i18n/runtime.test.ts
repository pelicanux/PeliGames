import {test, expect} from "bun:test";
import {browserLanguages, normalizeLocale, readLanguagePreference, resolveLanguage, type LanguageDefinition} from "../../src/i18n/preference";
import {createTranslationEngine, engine, loadLanguage, translator} from "../../src/i18n/engine";
import {languages} from "../../src/i18n/catalog.generated";

test("legacy choice, automatic preference and unavailable storage", () => {
  expect(readLanguagePreference()).toBe("auto");
  expect(readLanguagePreference({getItem: () => "pt"})).toBe("pt-BR");
  expect(readLanguagePreference({getItem: () => "en"})).toBe("en");
  expect(readLanguagePreference({getItem: () => {throw Error("blocked");}})).toBe("auto");
  expect(readLanguagePreference({getItem: () => "not_a_locale!!!!"})).toBe("auto");
});
test("system locale matching, preference order and English fallback", () => {
  expect(normalizeLocale("pt_BR.UTF-8")).toBe("pt-BR");
  expect(normalizeLocale("en_US@modifier")).toBe("en-US");
  expect(normalizeLocale("POSIX")).toBeUndefined();
  expect(resolveLanguage(["fr-FR", "pt-PT", "en-US"], languages)).toBe("pt-BR");
  expect(resolveLanguage(["de-DE"], languages)).toBe("en");
  expect(Array.isArray(browserLanguages())).toBe(true);
});
test("a third locale comes from metadata, including RTL and unpublished drafts", () => {
  const registry: LanguageDefinition[] = [...languages, {id: "ar", nativeName: "العربية", direction: "rtl", aliases: [], published: true}, {id: "es", nativeName: "Español", direction: "ltr", aliases: [], published: false}];
  expect(resolveLanguage(["ar-EG"], registry)).toBe("ar");
  expect(resolveLanguage(["es-ES"], registry)).toBe("en");
});
test("catalogues load once; original Portuguese labels stay available", async () => {
  await Promise.all([loadLanguage("pt-BR"), loadLanguage("pt-BR")]);
  const loaded = engine.getResourceBundle("pt-BR", "settings");
  await loadLanguage("pt-BR"); expect(engine.getResourceBundle("pt-BR", "settings")).toBe(loaded);
  expect(translator("pt-BR")("settings", "language")).toBe("Idioma");
  expect(translator("en")("settings", "language")).toBe("Language");
  await expect(loadLanguage("unsupported-locale")).rejects.toThrow("Unsupported language");
});
test("named variables, English fallback and CLDR plurals", () => {
  const instance = createTranslationEngine();
  instance.addResourceBundle("ar", "localization", {count_zero: "صفر {{count}}", count_one: "واحد {{count}}", count_two: "اثنان {{count}}", count_few: "قليل {{count}}", count_many: "كثير {{count}}", count_other: "آخر {{count}}"});
  const t = translator("ar", instance);
  expect(t("localization", "count", {count: 2})).toBe("اثنان 2");
  expect(t("localization", "count", {count: 7})).toBe("قليل 7");
  expect(t("settings", "language")).toBe("Language");
  expect(translator("en", instance)("localization", "count", {count: 1})).toBe("1 language available");
  expect(translator("en", instance)("localization", "count", {count: 3})).toBe("3 languages available");
});

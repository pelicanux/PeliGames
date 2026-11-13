import type {Resource} from "i18next";
import type {AddonI18n, AddonMessages, AddonTranslator} from "../../../addons/_infra/ui-sdk";
import {createTranslationEngine} from "../../i18n/engine";
import type {ResourceScope} from "./lifecycle";

/** Private catalogs using the same engine and language rules as the launcher. */
export function createAddonTranslator(language: string, catalog: AddonMessages) {
  const fallback = Intl.getCanonicalLocales(catalog.defaultLanguage)[0];
  const resources: Resource = {};
  const entries = Object.entries(catalog.resources);
  let count = 0, size = 0;
  if (!fallback || !entries.length || entries.length > 32) throw new Error("Invalid add-on language catalog");
  for (const [locale, messages] of entries) {
    const canonical = Intl.getCanonicalLocales(locale)[0];
    if (!canonical || resources[canonical] || !messages || typeof messages !== "object" || Array.isArray(messages)) throw new Error("Invalid add-on language catalog");
    const copied: Record<string, string> = Object.create(null);
    for (const [key, text] of Object.entries(messages)) {
      if (!key || typeof text !== "string" || ++count > 10000 || (size += key.length + text.length) > 1024 * 1024) throw new Error("Invalid or oversized add-on messages");
      copied[key] = text;
    }
    resources[canonical] = {addon: copied};
  }
  if (!resources[fallback]) throw new Error("Missing add-on default language");
  const instance = createTranslationEngine(resources, fallback, "addon");
  const translate: AddonTranslator = (key, values) => String(instance.t(key, {
    replace: values,
    count: typeof values?.count === "number" ? values.count : undefined,
    lng: language,
    ns: "addon",
  }));
  return {translate, dispose: () => {
    for (const locale of Object.keys(resources)) instance.removeResourceBundle(locale, "addon");
  }};
}
export function createAddonI18n(language: string, scope: ResourceScope): AddonI18n {
  return Object.freeze({language, createTranslator(catalog: AddonMessages): AddonTranslator {
    scope.assertOpen();
    const translator = createAddonTranslator(language, catalog);
    scope.onDispose(translator.dispose);
    return (key, values) => {
      scope.assertOpen();
      return translator.translate(key, values);
    };
  }});
}

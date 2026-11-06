import {createInstance, type i18n, type TOptions} from "i18next";
import {baseResources, languageLoaders} from "./catalog.generated";
import type {Catalog, Namespace} from "./catalog.generated";
export type {Catalog, Namespace};
export type TranslationValues = Record<string, unknown> & Pick<TOptions, "count">;
type PluralKey<K> = K extends `${infer Base}_${"zero" | "one" | "two" | "few" | "many" | "other"}` ? Base : never;
export type MessageKey<N extends Namespace> = (keyof Catalog[N] & string) | PluralKey<keyof Catalog[N] & string>;
export type Translator = <N extends Namespace>(namespace: N, key: MessageKey<N>, values?: TranslationValues) => string;
export function createTranslationEngine(): i18n {
  const instance = createInstance();
  void instance.init({
    lng: "en", fallbackLng: "en", initAsync: false, resources: {en: baseResources},
    defaultNS: "localization", ns: Object.keys(baseResources), keySeparator: false,
    interpolation: {escapeValue: false}, returnNull: false, returnEmptyString: false,
  });
  return instance;
}
export const engine = createTranslationEngine();
const pending = new Map<string, Promise<void>>();
export async function loadLanguage(language: string): Promise<void> {
  if (Object.keys(baseResources).every(namespace => engine.hasResourceBundle(language, namespace))) return;
  const load = languageLoaders[language];
  if (!load) throw new Error(`Unsupported language: ${language}`);
  let request = pending.get(language);
  if (!request) {
    request = load().then(module => {
      for (const [namespace, messages] of Object.entries(module.default)) engine.addResourceBundle(language, namespace, messages, true, true);
    }).catch(error => { pending.delete(language); throw error; });
    pending.set(language, request);
  }
  await request;
}
export function translator(language: string, instance: i18n = engine): Translator {
  return (namespace, key, values) => String(instance.t(String(key), {...values, lng: language, ns: namespace}));
}
/** Services receive a locale explicitly when they render outside a React component. */
export const translate = <N extends Namespace>(namespace: N, key: MessageKey<N>, values?: TranslationValues, language = engine.resolvedLanguage ?? "en"): string => translator(language)(namespace, key, values);

import {createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode} from "react";
import {I18nextProvider} from "react-i18next";
import {engine, loadLanguage, translator, type Catalog, type Namespace, type Translator} from "./engine";
import {languages} from "./catalog.generated";
import {browserLanguages, LANGUAGE_STORAGE_KEY, readLanguagePreference, resolveLanguage, type Language, type LanguagePreference} from "./preference";
import {systemLanguages} from "./systemLanguage";
interface I18nContextType {
  language: Language;
  preference: LanguagePreference;
  languages: typeof languages;
  loading: boolean;
  error: boolean;
  setLanguage: (language: LanguagePreference) => Promise<void>;
  t: Translator;
}
const Context = createContext<I18nContextType | undefined>(undefined);
function storage(): Storage | undefined { try { return typeof localStorage === "undefined" ? undefined : localStorage; } catch { return undefined; } }
export function I18nProvider({children, initialPreference}: {children: ReactNode; initialPreference?: LanguagePreference}) {
  const [preference, setPreference] = useState(() => initialPreference ?? readLanguagePreference(storage()));
  const [language, setResolved] = useState(() => resolveLanguage(preference === "auto" ? browserLanguages() : [preference], languages));
  const [loading, setLoading] = useState(false), [error, setError] = useState(false);
  const [ready, setReady] = useState(() => initialPreference !== undefined && Object.keys(engine.options.resources?.en ?? {}).every(namespace => engine.hasResourceBundle(language, namespace)));
  const revision = useRef(0), alive = useRef(true), isReady = useRef(ready);
  isReady.current = ready;
  const apply = useCallback(async (next: LanguagePreference, persist: boolean) => {
    const request = ++revision.current;
    setLoading(true); setError(false);
    try {
      const resolved = resolveLanguage(next === "auto" ? await systemLanguages() : [next], languages);
      await loadLanguage(resolved);
      if (!alive.current || request !== revision.current) return;
      await engine.changeLanguage(resolved);
      if (!alive.current || request !== revision.current) return;
      setResolved(resolved); setPreference(next); setReady(true);
      if (persist) { try { storage()?.setItem(LANGUAGE_STORAGE_KEY, next); } catch { /* The selection remains usable in memory. */ } }
    } catch {
      if (alive.current && request === revision.current) { if (!isReady.current) setResolved("en"); setError(true); setReady(true); }
    } finally { if (alive.current && request === revision.current) setLoading(false); }
  }, []);
  const currentPreference = useRef(preference); currentPreference.current = preference;
  useEffect(() => {
    alive.current = true;
    void apply(initialPreference ?? readLanguagePreference(storage()), false);
    const sync = (event: StorageEvent) => {
      if (event.storageArea && event.storageArea !== storage()) return;
      if (event.key === LANGUAGE_STORAGE_KEY || event.key === null) void apply(readLanguagePreference(storage()), false);
    };
    const systemChanged = () => { if (currentPreference.current === "auto") void apply("auto", false); };
    window.addEventListener("storage", sync); window.addEventListener("languagechange", systemChanged);
    return () => { alive.current = false; ++revision.current; window.removeEventListener("storage", sync); window.removeEventListener("languagechange", systemChanged); };
  }, [apply, initialPreference]);
  useEffect(() => {
    const definition = languages.find(item => item.id === language);
    document.documentElement.lang = language; document.documentElement.dir = definition?.direction ?? "ltr";
  }, [language]);
  const t = useMemo(() => translator(language), [language]);
  const setLanguage = useCallback((value: LanguagePreference) => apply(value === "pt" ? "pt-BR" : value, true), [apply]);
  const value = useMemo(() => ({language, preference, languages, loading, error, setLanguage, t}), [language, preference, loading, error, setLanguage, t]);
  return <I18nextProvider i18n={engine}><Context.Provider value={value}>{ready ? children : <div className="boot-splash" role="status" aria-label="Loading language"><img src="/peligames.svg" alt="PeliGames" /></div>}</Context.Provider></I18nextProvider>;
}
export function useI18n() {
  const value = useContext(Context);
  if (!value) throw new Error("useI18n must be used within an I18nProvider");
  return value;
}
/** Stable label objects preserve existing module APIs while sharing one catalogue. */
export function useMessages<N extends Namespace>(namespace: N): Catalog[N] {
  const {t, language} = useI18n();
  return useMemo(() => Object.fromEntries(Object.keys(engine.getResourceBundle("en", namespace) ?? {}).map(key => [key, t(namespace, key as keyof Catalog[N] & string)])) as Catalog[N], [namespace, t, language]);
}

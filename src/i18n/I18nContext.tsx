import React, { createContext, useContext, useEffect, useState, ReactNode } from "react";
import { translations, Language } from "./translations";

type TranslationKeys = keyof typeof translations.pt;
type SubKeys<T extends TranslationKeys> = keyof typeof translations.pt[T];

interface I18nContextType {
  language: Language;
  setLanguage: (lang: Language) => void;
  t: <T extends TranslationKeys>(section: T, key: SubKeys<T>) => string;
}

const I18nContext = createContext<I18nContextType | undefined>(undefined);
const LANGUAGE_STORAGE_KEY = "peligames.language";

export const I18nProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [language, updateLanguage] = useState<Language>(() => {
    try { return localStorage.getItem(LANGUAGE_STORAGE_KEY) === "en" ? "en" : "pt"; }
    catch { return "pt"; }
  });
  const setLanguage = (lang: Language) => {
    updateLanguage(lang);
    try { localStorage.setItem(LANGUAGE_STORAGE_KEY, lang); }
    catch { /* The current window can still change language without storage. */ }
  };
  useEffect(() => {
    document.documentElement.lang = language === "pt" ? "pt-BR" : "en";
  }, [language]);
  useEffect(() => {
    const sync = (event: StorageEvent) => {
      if (event.key === LANGUAGE_STORAGE_KEY || event.key === null) {
        updateLanguage(event.newValue === "en" ? "en" : "pt");
      }
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);

  const t = <T extends TranslationKeys>(section: T, key: SubKeys<T>): string => {
    return translations[language][section][key] as unknown as string;
  };

  return (
    <I18nContext.Provider value={{ language, setLanguage, t }}>
      {children}
    </I18nContext.Provider>
  );
};

export const useI18n = () => {
  const context = useContext(I18nContext);
  if (!context) {
    throw new Error("useI18n must be used within an I18nProvider");
  }
  return context;
};

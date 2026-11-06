/** Preferences and locale matching are independent of React and native IPC. */
export const LANGUAGE_STORAGE_KEY = "peligames.language";
export type Language = string;
export type LanguagePreference = "auto" | Language;
export interface LanguageDefinition { id: string; nativeName: string; direction: "ltr" | "rtl"; aliases: readonly string[]; published: boolean }
export function normalizeLocale(value: string): string | undefined {
  const raw = value.trim().split(/[.@]/)[0].replace(/_/g, "-");
  if (!raw || /^(C|POSIX)$/i.test(raw)) return undefined;
  try { return Intl.getCanonicalLocales(raw)[0]; } catch { return undefined; }
}
export function resolveLanguage(preferences: readonly string[], languages: readonly LanguageDefinition[]): string {
  const available = languages.filter(language => language.published);
  for (const preference of preferences) {
    const locale = normalizeLocale(preference);
    if (!locale) continue;
    const exact = available.find(language => language.id === locale || language.aliases.some(alias => normalizeLocale(alias) === locale));
    if (exact) return exact.id;
    const base = locale.split("-")[0];
    const sameBase = available.find(language => language.id === base) ?? available.find(language => language.id.split("-")[0] === base);
    if (sameBase) return sameBase.id;
  }
  return "en";
}
export function readLanguagePreference(storage?: Pick<Storage, "getItem">): LanguagePreference {
  try {
    const value = storage?.getItem(LANGUAGE_STORAGE_KEY);
    if (!value || value === "auto") return "auto";
    return value === "pt" ? "pt-BR" : normalizeLocale(value) ?? "auto";
  } catch { return "auto"; }
}
export function browserLanguages(): readonly string[] {
  if (typeof navigator === "undefined") return [];
  return navigator.languages?.length ? navigator.languages : navigator.language ? [navigator.language] : [];
}

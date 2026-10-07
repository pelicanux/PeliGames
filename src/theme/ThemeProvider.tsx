import { createContext, useContext, useEffect, useLayoutEffect, useState, type ReactNode } from "react";
import { ACCENT_STORAGE_KEY, APPEARANCE_STORAGE_KEY, DEFAULT_ACCENT, accentVariables, isAccentTheme, type AccentThemeId, type Appearance } from "./accentTheme";

const ThemeContext = createContext<{
  accent: AccentThemeId; setAccent: (id: AccentThemeId) => void;
  appearance: Appearance; setAppearance: (appearance: Appearance) => void;
} | null>(null);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [appearance, setAppearance] = useState<Appearance>(() => {
    try { return localStorage.getItem(APPEARANCE_STORAGE_KEY) === "light" ? "light" : "dark"; }
    catch { return "dark"; }
  });
  const [accent, setAccent] = useState<AccentThemeId>(() => {
    try {
      const saved = localStorage.getItem(ACCENT_STORAGE_KEY);
      return isAccentTheme(saved) ? saved : DEFAULT_ACCENT;
    } catch { return DEFAULT_ACCENT; }
  });
  useLayoutEffect(() => {
    for (const [property, value] of Object.entries(accentVariables(accent, appearance))) {
      document.documentElement.style.setProperty(property, value);
    }
    document.documentElement.dataset.accentTheme = accent;
    document.documentElement.dataset.appearance = appearance;
  }, [accent, appearance]);
  useEffect(() => {
    const sync = (event: StorageEvent) => {
      if (event.key === ACCENT_STORAGE_KEY) setAccent(isAccentTheme(event.newValue) ? event.newValue : DEFAULT_ACCENT);
      if (event.key === APPEARANCE_STORAGE_KEY) setAppearance(event.newValue === "light" ? "light" : "dark");
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, []);
  const chooseAccent = (value: AccentThemeId) => {
    setAccent(value);
    try { localStorage.setItem(ACCENT_STORAGE_KEY, value); } catch { /* Still usable without storage. */ }
  };
  const chooseAppearance = (value: Appearance) => {
    setAppearance(value);
    try { localStorage.setItem(APPEARANCE_STORAGE_KEY, value); } catch { /* Still usable without storage. */ }
  };
  return <ThemeContext.Provider value={{ accent, setAccent: chooseAccent, appearance, setAppearance: chooseAppearance }}>{children}</ThemeContext.Provider>;
}

export function useAccentTheme() {
  const context = useContext(ThemeContext);
  if (!context) throw new Error("useAccentTheme requires ThemeProvider");
  return context;
}

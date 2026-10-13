import { createContext, useContext, useEffect, useLayoutEffect, useState, type ReactNode } from "react";
import { useSurfaceNeonLighting } from "../hooks/useSurfaceNeonLighting";
import { DEFAULT_GLASS_EFFECTS, GLASS_STORAGE_KEY, readGlassEffects, type GlassEffects } from "./glassPreferences";
import { ACCENT_STORAGE_KEY, APPEARANCE_STORAGE_KEY, DEFAULT_ACCENT, accentVariables, isAccentTheme, type AccentThemeId, type Appearance } from "./accentTheme";

import { useDiscordBenefits } from "./useDiscordBenefits";
import { flushSync } from "react-dom";
import { InterfaceThemeEffects } from "./InterfaceThemeEffects";
import { animateThemeSurfaces, captureThemeSurfaces, HACKERMAN_ACCENT, INTERFACE_THEME_STORAGE_KEY, isFreeInterfaceTheme, readInterfaceTheme, type InterfaceTheme } from "./interfaceTheme";
import "./hackerman.css";

export type LaunchEffect = "shatter" | "smooth" | "aquarium" | "particles" | "penguin" | "shrink" | "freeze" | "disabled";
export const DEFAULT_LAUNCH_EFFECT: LaunchEffect = "smooth";
const launchEffectKey = "peligames_launch_effect";
const ThemeContext = createContext<{
  interfaceTheme: InterfaceTheme; setInterfaceTheme: (theme: InterfaceTheme) => void; themeTransitioning: boolean;
  exclusiveEffects: boolean; benefitExpiresAt: number | null;
  launchEffect: LaunchEffect; setLaunchEffect: (effect: LaunchEffect) => void;
  accent: AccentThemeId; setAccent: (id: AccentThemeId) => void;
  appearance: Appearance; setAppearance: (appearance: Appearance) => void;
  glassEffects: GlassEffects; setGlassEffects: (effects: GlassEffects) => void; resetGlassEffects: () => void;
} | null>(null);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const { exclusiveEffects, benefitExpiresAt, benefitsChecked } = useDiscordBenefits();
  const [preferredInterfaceTheme, updateInterfaceTheme] = useState(() => {
    try { return readInterfaceTheme(localStorage.getItem(INTERFACE_THEME_STORAGE_KEY)); } catch { return "standard" as InterfaceTheme; }
  });
  // Exclusive themes need the Discord benefit. The saved choice stays while the
  // saved entitlement is being read, so members never see the standard theme flash.
  const interfaceTheme: InterfaceTheme = exclusiveEffects || !benefitsChecked || isFreeInterfaceTheme(preferredInterfaceTheme) ? preferredInterfaceTheme : "standard";
  const [themeTransitioning, setThemeTransitioning] = useState(false);
  const chooseInterfaceTheme = (theme: InterfaceTheme) => {
    if (theme === interfaceTheme || themeTransitioning || (!exclusiveEffects && !isFreeInterfaceTheme(theme))) return;
    const apply = () => { flushSync(() => updateInterfaceTheme(theme)); try { localStorage.setItem(INTERFACE_THEME_STORAGE_KEY, theme); } catch { /* In-memory choice remains usable. */ } };
    if (matchMedia("(prefers-reduced-motion: reduce)").matches || document.body.classList.contains("performance-mode")) { apply(); return; }
    setThemeTransitioning(true);
    // Reveal the new theme from left to right, without extra visual overlays.
    const doc = document as Document & { startViewTransition?: (callback: () => void) => { finished: Promise<void> } };
    if (doc.startViewTransition) {
      document.documentElement.classList.add("interface-theme-reveal");
      const transition = doc.startViewTransition(apply);
      void transition.finished.catch(() => {}).finally(() => { document.documentElement.classList.remove("interface-theme-reveal"); setThemeTransitioning(false); });
    } else {
      const surfaces = captureThemeSurfaces();
      apply();
      animateThemeSurfaces(surfaces);
      window.setTimeout(() => setThemeTransitioning(false), 1800);
    }
  };
  const [preferredLaunchEffect, setLaunchEffect] = useState<LaunchEffect>(() => {
    try {
      const saved = localStorage.getItem(launchEffectKey);
      if (saved === "none") return "disabled";
      return saved === "shatter" || saved === "smooth" || saved === "aquarium" || saved === "particles" || saved === "penguin" || saved === "shrink" || saved === "freeze" || saved === "disabled" ? saved : DEFAULT_LAUNCH_EFFECT;
    } catch { return DEFAULT_LAUNCH_EFFECT; }
  });
  const launchEffect = exclusiveEffects || preferredLaunchEffect === "smooth" || preferredLaunchEffect === "disabled" ? preferredLaunchEffect : "smooth";
  const chooseLaunchEffect = (effect: LaunchEffect) => {
    if (!exclusiveEffects && effect !== "smooth" && effect !== "disabled") return;
    setLaunchEffect(effect);
    try { localStorage.setItem(launchEffectKey, effect); } catch { /* Keep the choice in memory. */ }
  };
  const [glassEffects, setGlassEffects] = useState(() => {
    try { return readGlassEffects(localStorage.getItem(GLASS_STORAGE_KEY)); }
    catch { return { ...DEFAULT_GLASS_EFFECTS }; }
  });
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
  const effectiveAppearance = interfaceTheme === "hackerman" ? "dark" : appearance;
  useSurfaceNeonLighting(effectiveAppearance);
  useLayoutEffect(() => {
    document.documentElement.style.setProperty("--window-blur-factor", String(glassEffects.windows / 50));
    document.documentElement.style.setProperty("--backdrop-blur-factor", String(glassEffects.backdrop / 50));
  }, [glassEffects]);
  useLayoutEffect(() => {
    for (const [property, value] of Object.entries({ ...accentVariables(accent, effectiveAppearance), ...(interfaceTheme === "hackerman" ? HACKERMAN_ACCENT : {}) })) {
      document.documentElement.style.setProperty(property, value);
    }
    document.documentElement.dataset.accentTheme = accent;
    document.documentElement.dataset.appearance = effectiveAppearance;
    document.documentElement.dataset.interfaceTheme = interfaceTheme;
  }, [accent, effectiveAppearance, interfaceTheme]);
  useEffect(() => {
    const sync = (event: StorageEvent) => {
      if (event.key === INTERFACE_THEME_STORAGE_KEY) updateInterfaceTheme(readInterfaceTheme(event.newValue));
      if (event.key === ACCENT_STORAGE_KEY) setAccent(isAccentTheme(event.newValue) ? event.newValue : DEFAULT_ACCENT);
      if (event.key === APPEARANCE_STORAGE_KEY) setAppearance(event.newValue === "light" ? "light" : "dark");
      if (event.key === GLASS_STORAGE_KEY) setGlassEffects(readGlassEffects(event.newValue));
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
  const chooseGlassEffects = (effects: GlassEffects) => {
    const next = readGlassEffects(JSON.stringify(effects));
    setGlassEffects(next);
    try { localStorage.setItem(GLASS_STORAGE_KEY, JSON.stringify(next)); } catch { /* Keep the active visual adjustment. */ }
  };
  return <ThemeContext.Provider value={{ interfaceTheme, setInterfaceTheme: chooseInterfaceTheme, themeTransitioning, exclusiveEffects, benefitExpiresAt, launchEffect, setLaunchEffect: chooseLaunchEffect, accent, setAccent: chooseAccent, appearance, setAppearance: chooseAppearance, glassEffects, setGlassEffects: chooseGlassEffects, resetGlassEffects: () => chooseGlassEffects(DEFAULT_GLASS_EFFECTS) }}>{children}<InterfaceThemeEffects active={interfaceTheme === "hackerman"} transitioning={themeTransitioning}/></ThemeContext.Provider>;
}

export function useAccentTheme() {
  const context = useContext(ThemeContext);
  if (!context) throw new Error("useAccentTheme requires ThemeProvider");
  return context;
}

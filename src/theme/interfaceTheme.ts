export type InterfaceTheme = "standard" | "hackerman" | "neon-wave";
export const INTERFACE_THEME_STORAGE_KEY = "peligames.interfaceTheme";
/** Only the standard theme is free; every other theme (Hackerman and any
 *  added later) needs the Discord benefit: YouTube member or server booster. */
export const isFreeInterfaceTheme = (theme: InterfaceTheme) => theme === "standard";
export const readInterfaceTheme = (value: string | null): InterfaceTheme => value === "hackerman" || value === "neon-wave" ? value : "standard";
export const HACKERMAN_ACCENT: Record<string, string> = {
  "--accent": "#269b54", "--accent-rgb": "38, 155, 84", "--accent-soft": "#8fc69e", "--accent-light": "#b1d9ba",
  "--accent-button": "#188445", "--accent-deep": "#125833", "--accent-hover": "#239855", "--accent-deeper": "#0e3b28",
  "--accent-foreground": "#d8f4dd", "--background-hue": "120deg", "--brand-hue": "-90deg",
};

/** Native WebKit fallback: colors change across visible controls from left to right. */
export function captureThemeSurfaces() {
  const keys = ["--accent", "--accent-rgb", "--accent-soft", "--accent-light", "--accent-button", "--accent-deep", "--accent-hover", "--accent-deeper", "--accent-foreground", "--glass-base-start", "--glass-base-end"];
  for (const name of keys) {
    const initialValue = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    try { if (initialValue && CSS.registerProperty) CSS.registerProperty({ name, syntax: name.endsWith("rgb") || name.startsWith("--glass-base") ? "<number>#" : "<color>", inherits: true, initialValue }); } catch { /* Already registered, or unsupported in this webview. */ }
  }
  return Array.from(document.querySelectorAll<HTMLElement>("button,input,textarea,select,h1,h2,h3,p,.menu-glass-panel,.modal-content,.dialog-titlebar,.app-wrapper"))
    .filter(element => { const rect = element.getBoundingClientRect(); return rect.width > 0 && rect.height > 0 && rect.bottom > 0 && rect.top < innerHeight; })
    .slice(0, 160).map(element => {
      const style = getComputedStyle(element);
      const variables = Object.fromEntries(keys.map(key => [key, style.getPropertyValue(key).trim()]).filter(([, value]) => value));
      return { element, left: element.getBoundingClientRect().left, old: { color: style.color, backgroundColor: style.backgroundColor, backgroundImage: style.backgroundImage, borderColor: style.borderColor, boxShadow: style.boxShadow, ...variables } };
    });
}
export function animateThemeSurfaces(surfaces: ReturnType<typeof captureThemeSurfaces>) {
  for (const { element, left, old } of surfaces) {
    if (!element.isConnected) continue;
    const style = getComputedStyle(element);
    const variables = Object.fromEntries(Object.keys(old).filter(key => key.startsWith("--")).map(key => [key, style.getPropertyValue(key).trim()]));
    const next = { color: style.color, backgroundColor: style.backgroundColor, backgroundImage: style.backgroundImage, borderColor: style.borderColor, boxShadow: style.boxShadow, ...variables };
    if (JSON.stringify(old) === JSON.stringify(next)) continue;
    element.animate([old, next], { delay: Math.max(0, Math.min(1, left / innerWidth)) * 1050, duration: 650, easing: "ease-out", fill: "backwards" });
  }
}

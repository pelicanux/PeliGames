export const ACCENT_THEMES = [
  { id: "red", color: "#ed1c24", hue: 0 },
  { id: "orange", color: "#f97316", hue: 35 },
  { id: "yellow", color: "#eab308", hue: 65 },
  { id: "green", color: "#22c55e", hue: 120 },
  { id: "blue", color: "#169bff", hue: 210 },
  { id: "indigo", color: "#6366f1", hue: 240 },
  { id: "violet", color: "#a855f7", hue: 275 },
] as const;

export type AccentThemeId = typeof ACCENT_THEMES[number]["id"];
export const DEFAULT_ACCENT: AccentThemeId = "blue";
export const ACCENT_STORAGE_KEY = "peligames.accentColor";
export type Appearance = "dark" | "light";
export const APPEARANCE_STORAGE_KEY = "peligames.appearance";
export function isAccentTheme(value: unknown): value is AccentThemeId {
  return ACCENT_THEMES.some(theme => theme.id === value);
}

export function accentVariables(id: AccentThemeId, appearance: Appearance = "dark"): Record<string, string> {
  const theme = ACCENT_THEMES.find(theme => theme.id === id)!;
  const rgb = [1, 3, 5].map(start => parseInt(theme.color.slice(start, start + 2), 16));
  const mix = (amount: number) => `rgb(${rgb.map(channel => Math.round(channel + (255 - channel) * amount)).join(", ")})`;
  const dark = (factor: number) => `rgb(${rgb.map(channel => Math.round(channel * factor)).join(", ")})`;
  const darkText = ["orange", "yellow", "green", "blue"].includes(id);
  return {
    "--accent": appearance === "light" ? dark(.72) : theme.color,
    "--accent-rgb": rgb.join(", "),
    "--accent-soft": appearance === "light" ? dark(.6) : mix(.42),
    "--accent-light": appearance === "light" ? dark(.6) : mix(.7),
    "--accent-button": dark(darkText ? 1 : .9),
    "--accent-deep": id === "red" ? "#a00b12" : dark(darkText ? .88 : .65),
    "--accent-hover": dark(darkText ? .97 : .85),
    "--accent-deeper": id === "red" ? "#7a080d" : dark(darkText ? .85 : .55),
    "--accent-foreground": darkText ? "#101522" : "#ffffff",
    "--background-hue": `${theme.hue}deg`,
    // The original pelican asset is blue; neutral whites and alpha stay intact.
    "--brand-hue": `${theme.hue - 210}deg`,
  };
}

export const GLASS_STORAGE_KEY = "peligames.glassEffects";
export const DEFAULT_GLASS_EFFECTS = { windows: 50, backdrop: 50 };
export type GlassEffects = typeof DEFAULT_GLASS_EFFECTS;

export function readGlassEffects(value: string | null): GlassEffects {
  try {
    const saved = JSON.parse(value || "null");
    const percent = (value: unknown) => typeof value === "number" && Number.isFinite(value)
      ? Math.round(Math.max(0, Math.min(100, value))) : 50;
    return { windows: percent(saved?.windows), backdrop: percent(saved?.backdrop) };
  } catch { return { ...DEFAULT_GLASS_EFFECTS }; }
}

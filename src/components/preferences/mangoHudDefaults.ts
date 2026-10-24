import catalog from "../../config/mangohudOptions.json";
import type { MangoHudSettings } from "../../services/mangohud";

/** Default value of every MangoHUD option in the catalog. */
export const mangoHudDefaults = () => Object.fromEntries(catalog.map(option => [option.key, option.default])) as MangoHudSettings;

/** Keys that are not appearance: the launcher switches, logging, the FPS limit and the keyboard shortcuts. */
const KEPT_ON_APPEARANCE_RESET = new Set([
  ...catalog.filter(option => option.section === "launcher" || option.section === "logs").map(option => option.key),
  "fps_limit", "fps_limit_method", "toggle_hud", "toggle_logging", "toggle_fps_limit", "reset_fps_metrics",
]);
/** `current` with only its appearance (what the preview shows) back to the defaults. */
export function mangoHudAppearanceDefaults(current: MangoHudSettings): MangoHudSettings {
  const next = { ...mangoHudDefaults() } as Record<string, unknown>;
  for (const key of KEPT_ON_APPEARANCE_RESET) if (key in current) next[key] = (current as Record<string, unknown>)[key];
  return next as MangoHudSettings;
}

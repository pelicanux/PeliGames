import catalog from "../../config/mangohudOptions.json";
import type { MangoHudSettings } from "../../services/mangohud";

/** Default value of every MangoHUD option in the catalog. */
export const mangoHudDefaults = () => Object.fromEntries(catalog.map(option => [option.key, option.default])) as MangoHudSettings;

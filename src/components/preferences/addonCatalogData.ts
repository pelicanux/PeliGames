import {translator} from "../../i18n/engine";
import {experimentalDanielNr} from "../../services/releaseFeatures";
export const addons = [
  {id: "daniel", label: "Daniel NR", icon: "neural", labelKey: "addon_daniel_0"},
  {id: "nexus", label: "Nexus Mods", icon: "puzzle", labelKey: "addon_nexus_0"},
] as const;
export type AddonId = typeof addons[number]["id"];
export const addonLabel = (id: AddonId, language: string) => { const tx = translator(language); return id === "daniel" ? (tx("addonCatalogData", "daniel_nr_test")) : addons.find(addon => addon.id === id)!.label; };

export const addonAvailable = (id: AddonId) => id !== "daniel" || experimentalDanielNr;

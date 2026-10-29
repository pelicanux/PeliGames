import type { GameInfo } from "../../components/GameGrid";
export type ReportCategory = "dlssnr" | "frame-generation" | "performance" | "captures" | "options" | "advanced";
export interface FeatureReportContext {
  category: ReportCategory; name: string; path: string; directory: string; mod_directory: string;
  capture_key: string; launcher: string; proton: string; nexus_id?: string;
  native: boolean; flatpak: boolean; advanced: GameInfo["advanced"] | null;
}
export function featureReportContext(game: GameInfo, category: ReportCategory, directory: string, modDirectory = ""): FeatureReportContext {
  return {category, name: game.name, path: game.path, directory, mod_directory: modDirectory,
    capture_key: game.external_id || game.path, launcher: game.launcher, proton: game.proton || game.steam_proton_name || "",
    nexus_id: game.nexus_id, native: Boolean(game.native_linux), flatpak: Boolean(game.flatpak), advanced: game.advanced ? structuredClone(game.advanced) : null};
}

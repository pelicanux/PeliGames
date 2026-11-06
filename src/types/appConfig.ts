/** Persisted launcher preferences and DLSSNR setup shared across feature modules. */
export interface AppConfig {
  steamgriddb_api_key?: string;
  scan_steam_protons?: boolean;
  backend: string;
  dll_version: string;
  shortcut_key: string;
  game_shortcut_keys?: Record<string, string>;
  custom_game_paths?: Record<string, string>;
  preferred_proton_family?: "ge-proton" | "cachyos-proton";
}

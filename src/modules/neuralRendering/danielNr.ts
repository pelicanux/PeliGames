import { invoke } from "@tauri-apps/api/core";
import type { GameInfo } from "../../components/GameGrid";
import { normalizeAdvanced, type AdvancedSettings } from "../../services/advancedSettings";

export interface DanielLoadingDllState {
  current: string | null;
  options: { name: string; available: boolean }[];
  error: string | null;
}
export interface DanielState {
  component_ready: boolean; package_ready: boolean; installed: boolean; enabled: boolean;
  directory: string; executable: string; wrapper: string; compiler: string | null;
  python_ready: boolean; gpu_access: boolean; conflict: string | null;
  version: string; component_version: string; loading_dll: DanielLoadingDllState;
  runtimes: { version: string; ready: boolean; present: boolean }[]; pending_version_change: boolean;
}
export interface DanielComponentState {
  runtimes: DanielState["runtimes"]; python_ready: boolean; component_version: string;
}
export const DANIEL_COMPONENT_CHANGED = "danielNrComponentChanged";
export const OPEN_DANIEL_PREFERENCES = "openDanielPreferences";
export const openDanielPreferences = () => window.dispatchEvent(new Event(OPEN_DANIEL_PREFERENCES));
export const loadDanielComponent = () => invoke<DanielComponentState>("daniel_nr_component_state");
export const operateDanielComponent = (operation: "prepare" | "import-runtime" | "remove-package", compiler: string, runtime: string, source?: string) =>
  invoke<DanielComponentState>("daniel_nr_operate", { game: null, operation, compiler: compiler || null, dx12: false, proxy: null, runtime, source: source || null });
export interface DanielGame { directory: string; executable: string | null; selected_executable: boolean; name: string; flatpak: boolean; }
export function danielGame(game: GameInfo, executable?: string): DanielGame {
  return { directory: game.directory || game.path, executable: executable || game.executable || null, selected_executable: Boolean(executable), name: game.name, flatpak: Boolean(game.flatpak) };
}
export const loadDaniel = (game: DanielGame) => invoke<DanielState>("daniel_nr_state", { game });
export const operateDaniel = (game: DanielGame, operation: "prepare" | "install" | "remove" | "enable" | "disable" | "switch-proxy" | "switch-version" | "recover-version" | "import-runtime", compiler: string, dx12: boolean, proxy?: string, runtime = "0.6.0", source?: string) =>
  invoke<DanielState>("daniel_nr_operate", { game, operation, compiler: compiler || null, dx12, proxy: proxy || null, runtime, source: source || null });
export function withDanielWrapper(settings: AdvancedSettings | undefined, wrapper: string, enabled: boolean, remove = false): AdvancedSettings {
  const advanced = normalizeAdvanced(settings);
  const wrappers = (advanced.wrappers || []).filter(item => item.program !== wrapper);
  return { ...advanced, wrappers: remove ? wrappers : [...wrappers, { program: wrapper, arguments: "", enabled }] };
}

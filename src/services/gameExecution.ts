import { invoke } from "@tauri-apps/api/core";
export interface ExecutionStatus {
  id: string; path: string; state: "starting" | "running" | "stopping" | "exited" | "cancelled" | "failed";
  log: string; error?: string | null;
}
export function isExecutionActive(status: ExecutionStatus | null) {
  return Boolean(status && ["starting", "running", "stopping"].includes(status.state));
}
async function previewRequest(body?: object): Promise<ExecutionStatus | null> {
  const response = await fetch("/__preview/game-execution", body ? { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) } : undefined);
  const result = await response.json();
  if (!response.ok || result.error) throw new Error(result.error || "Falha no monitor de execução.");
  return result.result;
}
export function executionStatus(): Promise<ExecutionStatus | null> {
  return "__PELI_UI_PREVIEW__" in window ? previewRequest() : invoke("get_peligames_execution");
}
export async function startGame(path: string, executable?: string): Promise<ExecutionStatus> {
  return "__PELI_UI_PREVIEW__" in window ? (await previewRequest({ action: "start", path, executable }))! : invoke("start_peligames_game", { path, executable });
}
/** Opens a program in the entry's prefix; null when it joined the prefix of the running game. */
export function runProgram(path: string, executable: string): Promise<ExecutionStatus | null> {
  return invoke("run_peligames_program", { path, executable });
}
/** A game a launcher installed in the prefix, by its shortcut. */
export interface LauncherShortcut { id: string; name: string; executable?: string | null; url?: string | null; icon?: string | null; in_library: boolean }
/** How a launcher's games get their own prefix, and which kinds its disk supports. */
export type ProfileKind = "overlay" | "reflink";
export interface ProfileOptions { kind: ProfileKind; overlay: boolean; reflink: boolean }
export function profileOptions(path: string): Promise<ProfileOptions> {
  return invoke("peligames_profile_options", { path });
}
export function setProfileKind(path: string, kind: ProfileKind): Promise<ProfileOptions> {
  return invoke("set_peligames_profile_kind", { path, kind });
}
export function addShortcutToLibrary(path: string, id: string): Promise<unknown> {
  return invoke("add_peligames_shortcut_to_library", { path, id });
}
export function removeShortcutFromLibrary(path: string, id: string): Promise<void> {
  return invoke("remove_peligames_shortcut_from_library", { path, id });
}
export function launcherShortcuts(path: string): Promise<LauncherShortcut[]> {
  return "__PELI_UI_PREVIEW__" in window ? Promise.resolve([]) : invoke("list_peligames_launcher_shortcuts", { path });
}
export function startShortcut(path: string, id: string): Promise<ExecutionStatus | null> {
  return invoke("start_peligames_shortcut", { path, id });
}
export async function cancelGame(id: string): Promise<ExecutionStatus> {
  return "__PELI_UI_PREVIEW__" in window ? (await previewRequest({ action: "cancel", id }))! : invoke("cancel_peligames_game", { id });
}

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
export async function cancelGame(id: string): Promise<ExecutionStatus> {
  return "__PELI_UI_PREVIEW__" in window ? (await previewRequest({ action: "cancel", id }))! : invoke("cancel_peligames_game", { id });
}

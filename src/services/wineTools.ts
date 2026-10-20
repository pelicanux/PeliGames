import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
export interface WinePackage { id: string; title: string; category: string; installed: boolean }
export interface WineToolResult { packages: WinePackage[]; log: string }
export async function runWineTool(path: string, action: "catalog" | "install" | "winecfg" | "winetricks-gui" | "regedit" | "explorer" | "uninstaller", packages: string[] = [], progress: (line: string) => void = () => {}, operationId?: string): Promise<WineToolResult> {
  const request = { path, action, packages };
  if (!("__PELI_UI_PREVIEW__" in window)) {
    const requestId = operationId ?? crypto.randomUUID();
    const stop = await listen<{id: string; line: string}>("peligames-wine-tool-progress", event => { if (event.payload.id === requestId) progress(event.payload.line); });
    try { return await invoke("run_wine_tool", { request, requestId }); } finally { stop(); }
  }
  const response = await fetch("/__preview/wine-tools", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(request) });
  if (!response.body) throw new Error("Serviço Wine indisponível.");
  const reader = response.body.getReader(), decoder = new TextDecoder(); let buffer = "", result: WineToolResult | undefined;
  const parse = (line: string) => { if (!line.trim()) return; const message = JSON.parse(line); if (message.error) throw new Error(message.error); if (message.progress) progress(message.progress); if (message.result) result = message.result; };
  try {
    while (true) { const {done,value} = await reader.read(); buffer += decoder.decode(value, {stream: !done}); const lines = buffer.split("\n"); buffer = lines.pop() ?? ""; for (const line of lines) parse(line); if (done) break; }
    parse(buffer); if (!response.ok || !result) throw new Error("A ferramenta encerrou sem resposta."); return result;
  } finally { reader.releaseLock(); }
}

export function isWineToolCancelled(error: unknown): boolean {
  return String(error).includes("PELIGAMES_WINE_TOOL_CANCELLED");
}
export async function cancelWineTool(requestId: string): Promise<void> {
  if ("__PELI_UI_PREVIEW__" in window) throw new Error("A prévia não executa instalações reais.");
  await invoke("cancel_wine_tool", {requestId});
}

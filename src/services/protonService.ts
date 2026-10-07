import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
export type ProtonFamily = "ge-proton" | "cachyos-proton";
export interface RunnerRelease {
  family: ProtonFamily; latest: string; current: string | null; installed_path: string | null;
  needs_update: boolean; release_url: string; asset: { name: string; size: number };
}
export interface RunnerProgress {
  family: ProtonFamily; phase: "downloading" | "verifying" | "extracting" | "complete";
  downloaded: number; total: number; speed_bytes_per_sec: number;
}
const preview = () => "__PELI_UI_PREVIEW__" in window;
export async function checkRunner(family: ProtonFamily): Promise<RunnerRelease> {
  if (!preview()) return invoke("check_proton_release", { family });
  const response = await fetch(`/__preview/runner/check?family=${family}`);
  const result = await response.json();
  if (!response.ok || result.error) throw new Error(result.error || "Falha ao consultar runner.");
  return result.result;
}
export async function installRunner(family: ProtonFamily, progress: (p: RunnerProgress) => void): Promise<string> {
  if (!preview()) {
    const unlisten = await listen<RunnerProgress>("proton-download-progress", e => { if (e.payload.family === family) progress(e.payload); });
    try { return await invoke("install_proton", { family }); } finally { unlisten(); }
  }
  const response = await fetch(`/__preview/runner/install?family=${family}`, { method: "POST" });
  if (!response.ok || !response.body) throw new Error((await response.json()).error || "Falha ao iniciar download.");
  const reader = response.body.getReader(), decoder = new TextDecoder(); let buffer = "", path: string | undefined;
  try {
    for (;;) {
      const chunk = await reader.read(); buffer += decoder.decode(chunk.value, { stream: !chunk.done });
      let newline: number;
      while ((newline = buffer.indexOf("\n")) >= 0) {
        const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1); if (!line.trim()) continue;
        const event = JSON.parse(line);
        if (event.error) throw new Error(event.error);
        if (event.progress) progress(event.progress);
        if (event.result?.path) path = event.result.path;
      }
      if (chunk.done) break;
    }
    if (!path) throw new Error("O serviço encerrou sem concluir a instalação.");
    return path;
  } finally { reader.releaseLock(); }
}
export async function cancelRunner(): Promise<void> {
  if (!preview()) return invoke("cancel_proton_install");
  const response = await fetch("/__preview/runner/cancel", { method: "POST" });
  if (!response.ok) throw new Error("Não foi possível solicitar o cancelamento.");
}

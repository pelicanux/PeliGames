// Persistent development bridge for the shared Rust execution supervisor.
// @ts-expect-error Node declarations are not installed in this frontend project.
import { spawn } from "node:child_process";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { existsSync } from "node:fs";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { join } from "node:path";
// @ts-expect-error Node declarations are not installed in this frontend project.
import process from "node:process";
import type { ExecutionStatus } from "../src/services/gameExecution";
let current: ExecutionStatus | null = null;
let cancelInput: ((text: string) => void) | null = null;
async function sharedExecution(args: string[]) {
  const binary = join(process.cwd(), "tools/proton-service/target/debug/peligames-proton-service");
  return new Promise<any>((resolve, reject) => {
    const child = spawn(binary, args, { stdio: ["ignore", "pipe", "pipe"] }); let output = "";
    child.stdout.on("data", (chunk: Uint8Array) => { output += String(chunk); });
    child.on("error", reject); child.on("close", (code: number) => {
      try { const result = JSON.parse(output.trim()); if (code || result.error) reject(new Error(result.error || "Falha ao consultar a execução.")); else resolve(result.result); } catch (error) { reject(error); }
    });
  });
}
export async function previewExecutionStatus() {
  if (current && ["starting", "running", "stopping"].includes(current.state)) return current;
  return await sharedExecution(["execution-status"]) || current;
}
export async function cancelPreviewGame(id: string) {
  if (!current || current.id !== id) return sharedExecution(["cancel-execution", id]);
  if (["starting", "running", "stopping"].includes(current.state)) {
    if (!cancelInput) throw new Error("O monitor da execução está indisponível.");
    cancelInput("cancel\n"); current = { ...current, state: "stopping" };
  }
  return current;
}
export function startPreviewGame(path: string, completed: () => void, executable?: string): Promise<ExecutionStatus> {
  const binary = join(process.cwd(), "tools/proton-service/target/debug/peligames-proton-service");
  if (!existsSync(binary)) throw new Error("Compile o serviço Rust da prévia: cargo build --manifest-path tools/proton-service/Cargo.toml");
  return new Promise((resolve, reject) => {
    const child = spawn(binary, ["monitor-game", path, ...(executable ? [executable] : [])], { stdio: ["pipe", "pipe", "pipe"] });
    let buffer = "", acknowledged = false, failure = "";
    cancelInput = text => { if (!child.stdin.destroyed) child.stdin.write(text); };
    child.stdout.on("data", (chunk: Uint8Array) => {
      buffer += String(chunk);
      let end: number;
      while ((end = buffer.indexOf("\n")) !== -1) {
        const line = buffer.slice(0, end); buffer = buffer.slice(end + 1);
        try {
          const message = JSON.parse(line);
          if (message.execution) {
            current = message.execution;
            if (!acknowledged) { acknowledged = true; resolve(current!); }
          }
          if (message.error) failure = message.error;
        } catch { failure = "Resposta inválida do monitor de execução."; }
      }
    });
    child.stderr.on("data", () => {});
    child.on("error", (error: Error) => { failure = error.message; });
    child.on("close", (code: number | null) => {
      cancelInput = null; completed();
      if (!acknowledged) { reject(new Error(failure || "Falha ao iniciar o monitor.")); return; }
      if (current && ["starting", "running", "stopping"].includes(current.state)) {
        current = { ...current, state: "failed", error: failure || `O monitor encerrou inesperadamente (${code}).` };
      }
    });
  });
}

export function previewGameLogs(path: string) { return sharedExecution(["game-logs", path]); }

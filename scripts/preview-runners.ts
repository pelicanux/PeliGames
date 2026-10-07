// Development transport only: catalog, downloads and extraction run in the shared Rust service.
// @ts-expect-error Node declarations are not installed in this frontend project.
import { spawn } from "node:child_process";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { existsSync } from "node:fs";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { join } from "node:path";
// @ts-expect-error Node declarations are not installed in this frontend project.
import process from "node:process";
import type { Plugin } from "vite";
let download: { stdin: { write: (text: string) => void }; on: Function } | null = null;
export function previewRunners(): Plugin {
  return { name: "peligames-preview-runners", apply: "serve", configureServer(server) {
    server.middlewares.use((request, response, next) => {
      const url = new URL(request.url ?? "/", "http://localhost");
      if (!url.pathname.startsWith("/__preview/runner/")) return next();
      const action = url.pathname.slice("/__preview/runner/".length), family = url.searchParams.get("family");
      const fail = (code: number, error: string) => { response.statusCode = code; response.setHeader("Content-Type", "application/json"); response.end(JSON.stringify({ error })); };
      response.setHeader("Cache-Control", "no-store");
      if (action !== "check" && (request.method !== "POST" || request.headers.origin !== `http://${request.headers.host}`)) return fail(403, "Solicitação inválida.");
      if (action === "cancel") { download?.stdin.write("cancel\n"); response.setHeader("Content-Type", "application/json"); response.end("{}"); return; }
      if (!["check", "install"].includes(action) || !["ge-proton", "cachyos-proton"].includes(family ?? "") || (action === "check" && request.method !== "GET")) return fail(400, "Runner inválido.");
      if (action === "install" && download) return fail(409, "Um runner já está sendo instalado.");
      const binary = join(process.cwd(), "tools/proton-service/target/debug/peligames-proton-service");
      if (!existsSync(binary)) return fail(503, "Compile o serviço Rust da prévia: cargo build --manifest-path tools/proton-service/Cargo.toml");
      const child = spawn(binary, [action, family!], { stdio: ["pipe", "pipe", "pipe"] });
      child.stdin.on("error", () => {});
      if (action === "install") {
        download = child;
        response.setHeader("Content-Type", "application/x-ndjson; charset=utf-8"); response.flushHeaders();
        child.stdout.on("data", (chunk: Uint8Array) => response.write(chunk));
        child.on("error", () => { response.write(JSON.stringify({ error: "Falha ao iniciar serviço Rust." }) + "\n"); response.end(); download = null; });
        child.on("close", () => { download = null; response.end(); });
        response.on("close", () => { if (!response.writableEnded) child.stdin.write("cancel\n"); });
      } else {
        let output = "";
        child.stdout.on("data", (chunk: Uint8Array) => { output += String(chunk); });
        child.on("error", () => fail(500, "Falha ao iniciar serviço Rust."));
        child.on("close", (code: number) => { response.statusCode = code === 0 ? 200 : 502; response.setHeader("Content-Type", "application/json"); response.end(output.trim() || JSON.stringify({ error: "Serviço Rust encerrou sem resposta." })); });
      }
      // Rust reports public errors over stdout. Drain stderr so the pipe cannot block.
      child.stderr.on("data", () => {});
    });
    server.httpServer?.on("close", () => { download?.stdin.write("cancel\n"); });
  } };
}

import { startPreviewGame, cancelPreviewGame, previewExecutionStatus, previewGameLogs } from "./preview-execution";
// Local transport only. Process preparation and execution stay in shared Rust.
// @ts-expect-error Node declarations are not installed in this frontend project.
import { spawn } from "node:child_process";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { existsSync } from "node:fs";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { join } from "node:path";
// @ts-expect-error Node declarations are not installed in this frontend project.
import process from "node:process";
import type { Plugin } from "vite";
let busy = false;
export function previewInstallation(): Plugin {
  return { name: "peligames-preview-installation", apply: "serve", configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (request.url?.startsWith("/__preview/game-logs?")) {
        response.setHeader("Content-Type", "application/json"); response.setHeader("Cache-Control", "no-store");
        try { const path = new URL(request.url, "http://localhost").searchParams.get("path"); if (!path) throw new Error("Jogo ausente."); response.end(JSON.stringify({result: await previewGameLogs(path)})); } catch (error) { response.statusCode=400; response.end(JSON.stringify({error:String(error)})); } return;
      }
      if (request.url === "/__preview/game-execution") {
        response.setHeader("Content-Type", "application/json"); response.setHeader("Cache-Control", "no-store");
        const fail = (code: number, error: string) => { response.statusCode = code; response.end(JSON.stringify({ error })); };
        if (request.method === "GET") { response.end(JSON.stringify({ result: await previewExecutionStatus() })); return; }
        if (request.method !== "POST" || request.headers.origin !== `http://${request.headers.host}`) return fail(403, "Solicitação inválida.");
        try {
          let body = "";
          for await (const chunk of request) { body += String(chunk); if (body.length > 16384) return fail(413, "Solicitação muito grande."); }
          const parsed = JSON.parse(body);
          if (parsed.action === "cancel" && typeof parsed.id === "string") {
            response.end(JSON.stringify({ result: await cancelPreviewGame(parsed.id) })); return;
          }
          if (parsed.action !== "start" || typeof parsed.path !== "string" || !parsed.path.trim() || (parsed.executable !== undefined && (typeof parsed.executable !== "string" || !parsed.executable.trim()))) return fail(400, "Configuração inválida.");
          if (busy) return fail(409, "Um jogo ou instalador já está em execução.");
          busy = true;
          try {
            const result = await startPreviewGame(parsed.path, () => { busy = false; }, parsed.executable);
            response.end(JSON.stringify({ result }));
          } catch (error) { busy = false; throw error; }
        } catch (error) { fail(500, error instanceof Error ? error.message : "Falha no monitor de execução."); }
        return;
      }
      const icon = request.url === "/__preview/pelinstall-icon";
      const matches = request.url === "/__preview/pelinstall-matches" || icon;
      const wine = request.url === "/__preview/wine-tools";
      const library = request.url === "/__preview/peligames-library";
      const add = request.url === "/__preview/peligames-add";
      const uninstall = request.url === "/__preview/peligames-uninstall";
      const settings = request.url === "/__preview/peligames-settings";
      const launch = request.url === "/__preview/game-launcher";
      if (!matches && !wine && !library && !launch && !settings && !add && !uninstall && request.url !== "/__preview/game-installer") return next();
      response.setHeader("Content-Type", "application/json"); response.setHeader("Cache-Control", "no-store");
      const fail = (code: number, error: string) => { if (!response.writableEnded) { response.statusCode = code; response.end(JSON.stringify({ error })); } };
      if (!(library && request.method === "GET") && (request.method !== "POST" || request.headers.origin !== `http://${request.headers.host}`)) return fail(403, "Solicitação inválida.");
      if (busy && !matches && (!library || request.method === "POST")) return fail(409, "Um instalador ou jogo já está em execução.");
      // Reserve before reading to prevent concurrent requests from both starting.
      if (!library && !matches) busy = true;
      let started = false;
      try {
        let body = "";
        for await (const chunk of request) { body += String(chunk); if (body.length > 16384) return fail(413, "Configuração muito grande."); }
        const parsed = library ? {} : JSON.parse(body);
        if (!library && !(matches ? typeof parsed.executable === "string" && parsed.executable.trim() : settings ? ["path", "name", "executable", "proton"].every(key => typeof parsed[key] === "string" && parsed[key].trim()) : (wine || launch || uninstall) ? typeof parsed.path === "string" && parsed.path.trim() : ["name", "directory", "executable", "proton"].every(key => typeof parsed[key] === "string" && parsed[key].trim()))) return fail(400, "Configuração incompleta.");
        const binary = join(process.cwd(), "tools/proton-service/target/debug/peligames-proton-service");
        if (!existsSync(binary)) return fail(503, "Compile o serviço Rust da prévia: cargo build --manifest-path tools/proton-service/Cargo.toml");
        const args = matches ? [icon ? "executable-icon" : "pelinstall-matches", parsed.executable] : wine ? ["wine-tools", JSON.stringify(parsed)] : add ? ["add-game", JSON.stringify(parsed)] : uninstall ? ["uninstall-game", parsed.path] : settings ? ["update-game", JSON.stringify(parsed)] : library ? ["list-games", request.method === "POST" ? "rescan" : "cached"] : launch ? ["launch-game", parsed.path] : ["run-game", JSON.stringify(parsed)];
        const child = spawn(binary, args, { stdio: ["ignore", "pipe", "pipe"] });
        started = true;
        if (wine) { response.setHeader("Content-Type", "application/x-ndjson; charset=utf-8"); response.flushHeaders(); }
        let output = "";
        child.stdout.on("data", (chunk: Uint8Array) => { if (wine) response.write(chunk); else output += String(chunk); });
        child.stderr.on("data", () => {});
        child.on("error", () => { if (!library && !matches) busy = false; fail(500, "Falha ao iniciar o serviço Rust."); });
        child.on("close", (code: number) => {
          if (!library && !matches) busy = false;
          if (response.writableEnded || response.destroyed) return;
          if (wine) { response.end(); return; }
          response.statusCode = code === 0 ? 200 : 502;
          response.end(output.trim() || JSON.stringify({ error: "Serviço encerrou sem resposta." }));
        });
        // Closing the preview does not terminate a Windows installer already running.
      } catch { fail(400, "Configuração inválida."); }
      finally { if (!started && !library && !matches) busy = false; }
    });
  } };
}

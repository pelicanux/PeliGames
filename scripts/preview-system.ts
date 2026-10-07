import type { Plugin } from "vite";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { readdir, readFile, stat, realpath, mkdir } from "node:fs/promises";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { homedir } from "node:os";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { join, isAbsolute } from "node:path";
// @ts-expect-error Node declarations are not installed in this frontend project.
import { execFile } from "node:child_process";
// @ts-expect-error Node declarations are not installed in this frontend project.
import process from "node:process";

export async function scanPreviewProtons(includeSteam = false) {
  const home = homedir();
  const libraries = new Set<string>(includeSteam ? [join(home, ".local/share/Steam"), join(home, ".steam/steam"), join(home, ".steam/root"), join(home, ".var/app/com.valvesoftware.Steam/.local/share/Steam")] : []);
  if (includeSteam && process.env.XDG_DATA_HOME) libraries.add(join(process.env.XDG_DATA_HOME, "Steam"));
  for (const library of [...libraries]) {
    for (const file of ["config/libraryfolders.vdf", "steamapps/libraryfolders.vdf"]) {
      try {
        const text = await readFile(join(library, file), "utf8");
        for (const match of text.matchAll(/"path"\s*"([^"]+)"/g)) libraries.add(match[1].replaceAll("\\\\", "/"));
      } catch { /* Missing Steam libraries are normal. */ }
    }
  }
  const config = process.env.XDG_CONFIG_HOME || join(home, ".config");
  const roots = [join(config, "peligames/runners/proton/ge-proton"), join(config, "peligames/runners/proton/cachyos-proton"),join(home, ".local/share/compatibilitytools.d"), join(home, ".local/share/lutris/runners/proton"), join(home, ".config/heroic/tools/proton"), join(home, ".var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton"), "/opt/proton"];
  if (process.env.XDG_DATA_HOME) roots.push(join(process.env.XDG_DATA_HOME, "lutris/runners/proton"));
  if (includeSteam) roots.push("/usr/share/steam/compatibilitytools.d", "/usr/local/share/steam/compatibilitytools.d");
  for (const library of libraries) roots.push(join(library, "compatibilitytools.d"), join(library, "steamapps/common"));
  const found = new Map<string, string>();
  for (const root of roots) {
    try {
      for (const name of await readdir(root)) {
        try {
          const path = join(root, name);
          if ((await stat(path)).isDirectory() && (await stat(join(path, "proton"))).isFile()) found.set(await realpath(path), name);
        } catch { /* Not a Proton installation. */ }
      }
    } catch { /* Optional directory. */ }
  }
  return [...found].map(([path, name]) => ({ name, path })).sort((a, b) => a.name.localeCompare(b.name));
}

let pickerBusy = false;
async function openFolder(path: string) {
  if (!isAbsolute(path) || path.includes("\0")) throw new Error("Caminho de pasta inválido.");
  const directory = await realpath(path);
  if (!(await stat(directory)).isDirectory()) throw new Error("A pasta selecionada não existe.");
  const env = { ...process.env };
  delete env.LD_LIBRARY_PATH;
  await new Promise<void>((resolve, reject) => {
    execFile("xdg-open", [directory], { env, maxBuffer: 8192 }, (error: Error | null) => {
      if (error) reject(new Error("Não foi possível abrir o gerenciador de arquivos do sistema."));
      else resolve();
    });
  });
}
function pick(kind: string): Promise<string | null> {
  const kde = String(process.env.XDG_CURRENT_DESKTOP).toLowerCase().includes("kde");
  const command = kde ? "kdialog" : "zenity";
  const args = kde ? kind === "directory" ? ["--getexistingdirectory", homedir(), "--title", "Local de instalação"]
    : ["--getopenfilename", homedir(), "*.exe *.msi|Executáveis Windows", "--title", "Executável do jogo / programa"]
    : ["--file-selection", ...(kind === "directory" ? ["--directory"] : ["--file-filter=Executáveis Windows | *.exe *.EXE *.msi *.MSI"]), "--title", kind === "directory" ? "Local de instalação" : "Executável do jogo / programa"];
  return new Promise((resolve, reject) => {
    execFile(command, args, { maxBuffer: 8192 }, (error: { code?: number } | null, stdout: string) => {
      if (error?.code === 1) resolve(null);
      else if (error) reject(error);
      else resolve(stdout.trim() || null);
    });
  });
}

export function previewSystem(): Plugin {
  return { name: "peligames-preview-system", apply: "serve", configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      const url = new URL(request.url ?? "/", "http://localhost");
      if (!["/__preview/protons", "/__preview/pick", "/__preview/installation-directory", "/__preview/open-folder"].includes(url.pathname)) return next();
      response.setHeader("Content-Type", "application/json; charset=utf-8");
      response.setHeader("Cache-Control", "no-store");
      if (url.pathname === "/__preview/open-folder") {
        if (request.method !== "POST" || request.headers.origin !== `http://${request.headers.host}`) {
          response.statusCode = 403; response.end(JSON.stringify({ error: "Solicitação inválida." })); return;
        }
        try {
          await openFolder(url.searchParams.get("path") || "");
          response.end(JSON.stringify({ success: true }));
        } catch (error) {
          response.statusCode = 500;
          response.end(JSON.stringify({ error: error instanceof Error ? error.message : "Não foi possível abrir a pasta." }));
        }
        return;
      }
      if (url.pathname === "/__preview/protons" && request.method === "GET") {
        try { response.end(JSON.stringify(await scanPreviewProtons(url.searchParams.get("steam") === "1"))); }
        catch { response.statusCode = 500; response.end(JSON.stringify({ error: "Não foi possível procurar Protons." })); }
        return;
      }
      if (url.pathname === "/__preview/installation-directory") {
        if (request.method !== "POST" || request.headers.origin !== `http://${request.headers.host}`) {
          response.statusCode = 403; response.end(JSON.stringify({ error: "Solicitação inválida." })); return;
        }
        try {
          const folder = url.searchParams.get("folder");
          if (folder !== null && (!folder || folder === "." || folder === ".." || /[/\\\u0000]/.test(folder))) {
            response.statusCode = 400; response.end(JSON.stringify({ error: "Nome de pasta inválido." })); return;
          }
          const path = join(homedir(), "Games", "PeliGames", ...(folder ? [folder] : []));
          await mkdir(path, { recursive: true });
          response.end(JSON.stringify({ path }));
        } catch { response.statusCode = 500; response.end(JSON.stringify({ error: "Não foi possível criar a pasta padrão." })); }
        return;
      }
      const kind = url.searchParams.get("kind");
      if (request.method !== "POST" || request.headers.origin !== `http://${request.headers.host}` || !["directory", "executable"].includes(kind ?? "")) {
        response.statusCode = 400; response.end(JSON.stringify({ error: "Solicitação inválida." })); return;
      }
      if (pickerBusy) { response.statusCode = 409; response.end(JSON.stringify({ error: "Um seletor já está aberto." })); return; }
      pickerBusy = true;
      try { response.end(JSON.stringify({ path: await pick(kind!) })); }
      catch { response.statusCode = 500; response.end(JSON.stringify({ error: "Não foi possível abrir o seletor do sistema." })); }
      finally { pickerBusy = false; }
    });
  } };
}

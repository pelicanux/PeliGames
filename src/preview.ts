// Development-only entry: simulated installation data and real public-catalog artwork.
// The native application's main entry never imports this module.
import type { GameInfo } from "./components/GameGrid";

if (!import.meta.env.DEV) throw new Error("A prévia está disponível apenas no servidor de desenvolvimento.");

const games: GameInfo[] = [
  { name: "Jogo com mod", path: "/preview/installed", app_id: "1", launcher: "Steam", cover_url: "/peligames.png" },
  { name: "Jogo sem mod", path: "/preview/empty", app_id: "2", launcher: "Steam", cover_url: "/peligames.png" },
  { name: "Jogo Heroic", path: "/preview/heroic", launcher: "Heroic", cover_url: "/peligames.png" },
];

localStorage.setItem("game_library_cache_v1", JSON.stringify({ version: 1, games }));
let callbackId = 0;
Object.assign(window, {
  __PELI_UI_PREVIEW__: true,
  __TAURI_INTERNALS__: {
    transformCallback: () => ++callbackId,
    unregisterCallback: () => {},
    invoke: async (command: string, args: Record<string, unknown> = {}) => {
      switch (command) {
        case "get_startup_context": {
          const params = new URLSearchParams(location.search);
          const module = params.get("module");
          return { entry: params.get("showGame"), module: module === "pelinstall" || module === "launch-error" ? module : "launcher", executable: module === "pelinstall" ? params.get("executable") || "/preview/instalador.exe" : null, error: module === "launch-error" ? "Exemplo de falha do Proton. No aplicativo, o monitor usa o estado real do processo e seu log." : null };
        }
        case "load_app_config": return JSON.parse(localStorage.getItem("peligames-preview-config") || '{"backend":"AMDNR","dll_version":"preview.bin","shortcut_key":"Insert"}');
        case "save_app_config": {
          const previous = JSON.parse(localStorage.getItem("peligames-preview-config") || "{}");
          localStorage.setItem("peligames-preview-config", JSON.stringify({ ...previous, ...args.config as object }));
          return null;
        }
        case "scan_installed_games": return games;
        case "open_folder": {
          const response = await fetch(`/__preview/open-folder?path=${encodeURIComponent(String(args.path ?? ""))}`, { method: "POST" });
          const result = await response.json();
          if (!response.ok) throw new Error(result.error || "Não foi possível abrir a pasta.");
          return null;
        }
        case "launch_game": {
          if (args.launcher !== "PeliGames") return null;
          const response = await fetch("/__preview/game-launcher", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ path: args.path }) });
          const result = await response.json();
          if (!response.ok || result.error) throw new Error(result.error || "Falha ao iniciar o jogo.");
          return null;
        }
        case "prepare_default_installation_directory": {
          const folder = typeof args.folder === "string" ? `?folder=${encodeURIComponent(args.folder)}` : "";
          const response = await fetch(`/__preview/installation-directory${folder}`, { method: "POST" });
          const result = await response.json();
          if (!response.ok) throw new Error(result.error);
          return result.path;
        }
        case "scan_installed_protons": {
          const config = JSON.parse(localStorage.getItem("peligames-preview-config") || "{}");
          const response = await fetch(`/__preview/protons?steam=${config.scan_steam_protons ? "1" : "0"}`);
          if (!response.ok) throw new Error("Não foi possível procurar Protons.");
          return response.json();
        }
        case "plugin:dialog|open": {
          const options = args.options as { directory?: boolean };
          const kind = options?.directory ? "directory" : "executable";
          const response = await fetch(`/__preview/pick?kind=${kind}`, { method: "POST" });
          if (!response.ok) throw new Error("Não foi possível abrir o seletor do sistema.");
          const result: { path: string | null } = await response.json();
          return result.path;
        }
        case "get_game_installation_details": return args.gameDir === "/preview/installed"
          ? { status: "Vulkan/DX9", installed_dll: "vulkan-1.dll" }
          : { status: "Nenhum", installed_dll: null };
        case "analyze_game": return { platform: "Windows (Proton / Wine)", architecture: "32-bits", graphics_api: "DirectX 9", upscalers: [] };
        case "fetch_steamgriddb_cover_command": {
          const response = await fetch(`/__preview/cover?name=${encodeURIComponent(String(args.name))}`);
          if (!response.ok) throw new Error("Não foi possível buscar a capa na prévia.");
          const result: { cover: string | null } = await response.json();
          return result.cover;
        }
        case "fetch_steam_release_date": return "15 Dec, 2009";
        case "get_neural_startup": return true;
        case "check_launcher_update": return { current: __APP_VERSION__, latest: __APP_VERSION__, available: false, notes: "", packages: [], preferred: null, appimage: false, format: "" };
        case "plugin:event|listen": return ++callbackId;
        case "plugin:event|unlisten":
        case "log_cached_library": return null;
        default: throw new Error("Prévia visual: esta ação precisa do aplicativo desktop e não será executada no navegador.");
      }
    },
  },
});

await import("./main");

// Demo-only state: never invokes Proton, mounts profiles, or writes game files.
import type { GameInfo } from "./components/GameGrid";
import type { ExecutionStatus, LauncherShortcut, ProfileKind } from "./services/gameExecution";
import type { LauncherGame } from "./services/installedLibrary";
export const demoLauncher: GameInfo = { name: "EA app — demonstração", path: "peligames:preview-ea", executable: "/preview/ea-prefix/drive_c/EA/EAapp.exe", directory: "/preview/ea", prefix: "/preview/ea-prefix", proton: "/preview/proton/Proton-GE Latest", launcher: "PeliGames", cover_url: "/images/preview-launcher.svg" };
const shortcuts: LauncherShortcut[] = [
  { id: "demo-apex", name: "Apex Legends", executable: "/preview/ea-prefix/drive_c/Games/Apex/r5apex.exe", in_library: false },
  { id: "demo-it-takes-two", name: "It Takes Two", executable: "/preview/ea-prefix/drive_c/Games/ItTakesTwo/ItTakesTwo.exe", in_library: false },
];
let kind: ProfileKind = "overlay";
let session: ExecutionStatus | null = null;
let offerGames = false;
const declined = new Set<string>();
const json = (result: unknown) => new Response(JSON.stringify({ result }), { headers: { "Content-Type": "application/json" } });
const start = (path: string) => (session = { id: crypto.randomUUID(), path, state: "running", log: "Execução simulada: nenhum programa real foi aberto." });
function startPrepared(path: string) {
  const started = start(path); session = { ...started, state: "starting" };
  window.setTimeout(() => { if (session?.id === started.id && session.state === "starting") session = { ...session, state: "running" }; }, 1800);
  return session;
}
export function simulateLauncherDiscovery() {
  const started = start(demoLauncher.path);
  window.setTimeout(() => { if (session?.id === started.id) { offerGames = true; session = { ...started, state: "exited" }; } }, 1800);
}
function addProgram(id: string, games: GameInfo[]) {
  const item = shortcuts.find(entry => entry.id === id);
  if (!item) throw new Error("Atalho de demonstração não encontrado.");
  item.in_library = true;
  const entry = { ...demoLauncher, name: item.name, path: `peligames:preview-${id}`, executable: item.executable!, prefix: `/preview/ea/profiles/${id}/prefix`, cover_url: `https://cdn.cloudflare.steamstatic.com/steam/apps/${id === "demo-apex" ? "1172470" : "1426210"}/library_600x900.jpg` };
  const index = games.findIndex(game => game.path === entry.path);
  if (index < 0) games.push(entry); else games[index] = entry;
  return entry;
}
export function demoLauncherInvoke(command: string, args: Record<string, unknown>, games: GameInfo[]): { value: unknown } | undefined {
  switch (command) {
    case "list_peligames_launcher_shortcuts": return { value: args.path === demoLauncher.path ? shortcuts.map(item => ({ ...item })) : [] };
    case "peligames_profile_options": return { value: { kind, overlay: true, reflink: true } };
    case "set_peligames_profile_kind": kind = args.kind === "reflink" ? "reflink" : "overlay"; return { value: { kind, overlay: true, reflink: true } };
    case "add_peligames_shortcut_to_library": return { value: addProgram(String(args.id), games) };
    case "remove_peligames_shortcut_from_library": {
      const item = shortcuts.find(entry => entry.id === args.id); if (item) item.in_library = false;
      const index = games.findIndex(game => game.path === `peligames:preview-${args.id}`); if (index >= 0) games.splice(index, 1);
      return { value: null };
    }
    case "run_peligames_program":
    case "start_peligames_shortcut": return { value: session?.state === "running" ? null : startPrepared(String(args.path)) };
    case "get_pelinstall_icon": return { value: null };
  }
}
export function demoLauncherFetch(input: string, init: RequestInit | undefined, games: GameInfo[]): Response | Promise<Response> | undefined {
  if (input === "/__preview/wine-tools") {
    const request = JSON.parse(String(init?.body || "{}"));
    if (request.action !== "catalog") {
      const mode = new URLSearchParams(location.search).get("wineTools");
      if (["winecfg", "regedit", "explorer"].includes(request.action) && (mode === "slow" || mode === "error")) {
        return new Promise(resolve => setTimeout(() => resolve(mode === "error"
          ? new Response(JSON.stringify({ error: "Falha simulada ao abrir a ferramenta Wine." }), { status: 400 })
          : json({ packages: [], log: "Prévia: nenhuma ferramenta Wine foi aberta." })), 5000));
      }
      return new Response(JSON.stringify({ error: "Prévia visual: nenhuma ferramenta Wine será executada." }), { status: 400 });
    }
    const packages = [
      { id: "vcrun2022", title: "Microsoft Visual C++ 2015–2022", category: "dlls", installed: false },
      { id: "d3dx9", title: "Bibliotecas DirectX 9", category: "dlls", installed: false },
      { id: "corefonts", title: "Fontes básicas Microsoft", category: "fonts", installed: true },
    ];
    if (new URLSearchParams(location.search).get("winetricks") === "large") {
      for (let index = 0; index < 1000; index++) packages.push({ id: `component_${index}`, title: `Componente de demonstração ${index}`, category: "dlls", installed: false });
    }
    return json({ packages, log: "Catálogo de demonstração; nenhum arquivo foi alterado." });
  }
  if (input === "/__preview/game-execution") {
    if (!init?.body) return json(session);
    const body = JSON.parse(String(init.body));
    if (body.action === "start") return json(startPrepared(body.path));
    if (body.action === "cancel" && session && session.id === body.id) { if (session.path === demoLauncher.path) offerGames = true; session = { ...session, state: "cancelled" }; }
    return json(session);
  }
  if (input === "/__preview/peligames-launcher-games") {
    const { path } = JSON.parse(String(init?.body || "{}"));
    return json(path === demoLauncher.path && offerGames ? shortcuts.filter(item => !item.in_library && !declined.has(item.executable!)).map(item => ({ name: item.name, executable: item.executable })) : []);
  }
  if (input === "/__preview/peligames-launcher-add") {
    const body = JSON.parse(String(init?.body || "{}"));
    for (const executable of body.declined || []) declined.add(executable);
    const added = (body.added as LauncherGame[] || []).map(item => addProgram(shortcuts.find(entry => entry.executable === item.executable)!.id, games));
    offerGames = false;
    return json(added);
  }
  if (input === "/__preview/peligames-settings") {
    const body = JSON.parse(String(init?.body || "{}"));
    const index = games.findIndex(game => game.path === body.path);
    if (index < 0) return undefined;
    games[index] = { ...games[index], ...body };
    return json(games[index]);
  }
}

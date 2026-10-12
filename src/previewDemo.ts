import { demoLauncher, demoLauncherInvoke } from "./previewLauncherDemo";
import { orderWorkspaceMods } from "./modules/nexus/dependencyOrder";
// Visual fixtures only: no Nexus account, downloads or game files are accessed.
import type { GameInfo } from "./components/GameGrid";
import type { NexusGame } from "./modules/nexus/useNexusWorkspace";
import type { ProfileSummary } from "./modules/nexus/NexusProfiles";
import type { NexusDownload } from "./modules/nexus/NexusDownloadsPanel";
const specs = [
  ["Stardew Valley", "413150", "stardewvalley"],
  ["Palworld", "1623730", "palworld"],
  ["Cyberpunk 2077", "1091500", "cyberpunk2077"],
  ["Final Fantasy XII The Zodiac Age", "595520", "finalfantasy12"],
  ["Baldur's Gate 3", "1086940", "baldursgate3"],
  ["Tomb Raider", "203160", "tombraider"],
  ["Fallout: New Vegas", "22380", "newvegas"],
];
export const demoGames: GameInfo[] = specs.map(([name, id], i) => ({ name, app_id: id, path: `/preview/game-${i}/game.exe`, directory: `/preview/game-${i}`, launcher: i > 1 && !["203160", "22380"].includes(id) ? "PeliGames" : "Steam", cover_url: `https://cdn.cloudflare.steamstatic.com/steam/apps/${id}/library_600x900.jpg` }));
// Three candidates for one example installation; other demo games have only one.
const demoExecutableCandidates = [
  { ...demoGames[2], executable: demoGames[2].path, recommended: true, discovery_source: "Atalho do Windows" },
  { ...demoGames[2], path: "/preview/game-2/launcher.exe", executable: "/preview/game-2/launcher.exe" },
  { ...demoGames[2], path: "/preview/game-2/game_dx12.exe", executable: "/preview/game-2/game_dx12.exe" },
];
export function demoExecutableChoices(path: string): GameInfo[] {
  return demoExecutableCandidates.some(entry => entry.path === path) ? demoExecutableCandidates : [];
}
export function demoChooseExecutable(path: string): GameInfo | undefined {
  const chosen = demoExecutableCandidates.find(entry => entry.path === path);
  if (chosen) demoGames[2] = { ...chosen };
  return chosen;
}
const workspace: { games: NexusGame[] } = { games: demoGames.map((game, i) => ({ id: `demo-${i}`, game, platform: specs[i][2] === "stardewvalley" ? "native" : "proton", adapter: specs[i][2] === "tombraider" ? "" : specs[i][2], nexus_domain: specs[i][2], running: false, compat_data: i === 0 ? "" : `/preview/prefix-${i}`, proton: i === 0 ? "" : "/preview/proton/Proton-GE Latest", mods: specs[i][2] === "tombraider" ? [] : [
  { id: "demo-installed", name: "Mod visual — exemplo ativo", version: "2.0.0", archive: "/preview/visual.zip", size: 2048000, installed: true, enabled: true },
  { id: "demo-unstructured", name: "Mod sem estrutura — exemplo", version: "1.0.0", archive: "/preview/sem-estrutura.zip", size: 1024000, installed: false, enabled: false },
  { id: "demo-imported", name: "Mod de interface — exemplo importado", version: "1.2.0", archive: "/preview/interface.zip", size: 4096000, installed: false, enabled: false, nexus_source: { domain: specs[i][2], mod_id: 3, file_id: 30, requirements: { complete: true, items: i === 2 ? [{name:"TweakXL",kind:"nexus",notes:"",url:"https://www.nexusmods.com/cyberpunk2077/mods/4197"}] : [] } } },
  ...(i === 2 ? [
    {id:"demo-tweakxl",name:"TweakXL",archive:"/preview/TweakXL.zip",size:1045504,installed:false,enabled:false,nexus_source:{domain:"cyberpunk2077",mod_id:4197,file_id:41970,requirements:{complete:true,items:[{name:"RED4ext",notes:"",kind:"nexus" as const,url:"https://www.nexusmods.com/cyberpunk2077/mods/2380"}]}}},
    {id:"demo-red4ext",name:"RED4ext",archive:"/preview/RED4ext.zip",size:575488,installed:false,enabled:false,nexus_source:{domain:"cyberpunk2077",mod_id:2380,file_id:23800,requirements:{complete:true,items:[]}}},
  ] : []),
] })) };
demoGames.push(demoLauncher);
const demoProfiles = new Map<string, ProfileSummary[]>();
function profilesFor(gameId: string) {
  if (!demoProfiles.has(gameId)) demoProfiles.set(gameId, [
    { id: "demo-default", name: "Padrão", active: true, enabled: 1, own_saves: false, own_settings: false, saves_supported: true },
    ...(new URLSearchParams(location.search).get("profiles") === "1" ? [{ id: "demo-extra", name: "Perfil de teste", active: false, enabled: 3, own_saves: true, own_settings: false, saves_supported: true }] : []),
  ]);
  return demoProfiles.get(gameId)!;
}
let tombRaiderAssociated = false;
let jobs: NexusDownload[] = [];
const demoLoadOrder = {
  entries: ["Interface de exemplo", "Dados de exemplo"].map((name, i) => ({
    file: `demo-bg3-${i}.pak`, name, uuid: `demo-bg3-${i}`, locked: false,
    managed: "Demonstração", missing: [] as string[], late: [] as string[],
  })),
  auto_export: false, format: "v8", mod_fixer: false,
  settings: "/preview/prefix-4/AppData/Local/Larian Studios/Baldur's Gate 3/PlayerProfiles/Public/modsettings.lsx",
};
const sleep = (ms = 400) => new Promise(resolve => window.setTimeout(resolve, ms));
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value));
function catalog(domain: string) {
  const spec = specs.find(s => s[2] === domain) || specs[0];
  const names = domain === "stardewvalley" ? ["NPC Map Locations", "SMAPI", "Seasonal Outfits", "Content Patcher"] : ["Melhorias de interface", "Framework do jogo", "Pacote visual", "Configurações do mod"];
  return names.map((name, i) => ({ mod_id: i + 1, name, author: "Conta de demonstração", version: "1.2.0", summary: `Prévia visual para ${spec[0]}. Dados simulados para testar os controles do catálogo.`, description: "Demonstração: selecione um arquivo e os requisitos para acompanhar a confirmação e o progresso do download. Nenhum arquivo será baixado ou aplicado.", downloads: 120000 - i * 18000, picture_url: `https://cdn.cloudflare.steamstatic.com/steam/apps/${spec[1]}/header.jpg` }));
}
function details(domain: string, id: number) {
  const info = catalog(domain).find(m => m.mod_id === id) || catalog(domain)[0];
  return { info, requirements: { complete: true, items: (id === 1 || id === 3) ? [{ name: catalog(domain)[3].name, notes: "Necessário para este exemplo de mod.", kind: "nexus", url: `https://www.nexusmods.com/${domain}/mods/4` }] : [] }, files: [
    { file_id: id * 10 + 4, name: "Configuração adicional", file_name: "optional-config.zip", version: "1.0.0", size: 1024, category_id: 3, category_name: "OPTIONAL", description: "Arquivo opcional de demonstração." },
    { file_id: id * 10 + 5, name: "Pacote visual extra", file_name: "optional-visual.zip", version: "1.0.0", size: 2048, category_id: 3, category_name: "OPTIONAL", description: "Outro opcional para testar a seleção independente." },
    { file_id: id * 10, name: info.name, file_name: `${info.name.replace(/ /g, "_")}.zip`, version: "1.2.0", size: 8192, category_id: 1, category_name: "MAIN", description: "Versão principal — exemplo de descrição do arquivo." },
    { file_id: id * 10 + 1, name: info.name, file_name: "old-version.zip", version: "1.0.0", size: 4096, category_id: 4, category_name: "OLD_VERSION", description: "Versão antiga, apenas para testar a expansão." },
    { file_id: id * 10 + 2, name: info.name, file_name: "old-version-0.9.zip", version: "0.9.0", size: 3072, category_id: 4, category_name: "OLD_VERSION", description: "Outra versão antiga de demonstração." },
    { file_id: id * 10 + 3, name: info.name, file_name: "old-version-0.8.zip", version: "0.8.0", size: 2048, category_id: 4, category_name: "OLD_VERSION", description: "Arquivo antigo simulado para conferir as colunas." },
  ] };
}
function advance() {
  if (jobs.some(j => ["authorizing", "waiting"].includes(j.status))) return;
  const next = jobs.find(j => j.status === "queued");
  if (next) next.status = "authorizing";
}
function confirm() {
  const job = jobs.find(j => j.status === "authorizing");
  if (!job) return;
  job.status = "downloading";
  window.setTimeout(advance, 1500);
  const timer = window.setInterval(() => {
    if (job.status !== "downloading") { window.clearInterval(timer); return; }
    job.received = Math.min(job.total!, job.received + 1048576);
    if (job.received === job.total) {
      window.clearInterval(timer); job.status = "importing";
      window.setTimeout(() => {
        if (job.status !== "importing") return;
        job.status = "imported"; job.mod_entry_id = job.id;
        const game = workspace.games.find(g => g.id === job.game_id);
        game?.mods.push({ id: job.id, name: details(job.domain,job.mod_id).info.name, version: details(job.domain,job.mod_id).files.find(file => file.file_id === job.file_id)?.version, archive: `/preview/${job.name}`, size: job.total!, installed: false, enabled: false });
        advance();
      }, 1000);
    }
  }, 2000);
}
export async function demoInvoke(command: string, args: Record<string, unknown>): Promise<{ value: unknown } | undefined> {
  const launcherResult = demoLauncherInvoke(command, args, demoGames);
  if (launcherResult) return launcherResult;
  workspace.games = orderWorkspaceMods(workspace).games;
  const domain = String(args.gameDomain || "stardewvalley");
  switch (command) {
    case "nexus_user_profiles": return { value: clone(profilesFor(String(args.gameId))) };
    case "create_nexus_user_profile": {
      const profiles = profilesFor(String(args.gameId));
      profiles.push({ id: crypto.randomUUID(), name: String(args.name), active: false, enabled: args.copy ? profiles.find(profile => profile.active)?.enabled || 0 : 0, own_saves: false, own_settings: false, saves_supported: true });
      return { value: clone(profiles) };
    }
    case "rename_nexus_user_profile":
    case "set_nexus_profile_options": {
      const profiles = profilesFor(String(args.gameId)), profile = profiles.find(profile => profile.id === args.id);
      if (profile && command === "rename_nexus_user_profile") profile.name = String(args.name);
      if (profile && command === "set_nexus_profile_options") { profile.own_saves = Boolean(args.ownSaves); profile.own_settings = Boolean(args.ownSettings); }
      return { value: clone(profiles) };
    }
    case "delete_nexus_user_profile": {
      const profiles = profilesFor(String(args.gameId)).filter(profile => profile.active || profile.id !== args.id);
      demoProfiles.set(String(args.gameId), profiles);
      return { value: clone(profiles) };
    }
    case "switch_nexus_user_profile": {
      for (const profile of profilesFor(String(args.gameId))) profile.active = profile.id === args.id;
      const game = workspace.games.find(game => game.id === args.gameId);
      if (game) game.profile = String(args.id);
      return { value: clone(workspace) };
    }
    case "prepare_default_installation_directory": return {value: `/preview/games${typeof args.folder === "string" && args.folder ? `/${args.folder}` : ""}`};
    case "read_nexus_mod_logs": return {value:`Demonstração — registros simulados\n--- red4ext/logs/red4ext.log | modified_unix=${Math.floor(Date.now()/1000)} ---\n[RED4ext] Plugin TweakXL loaded\n[TweakXL] Loading tweaks: example.yaml`};
    case "read_nexus_logs": return {value: args.previous ? "Demonstração — execução anterior\nJogo encerrado." : "Demonstração — execução atual\nIniciando o jogo com mods."};
    case "scan_installed_protons": {
      const installed = [...demoRunnerInstalled].map(family => {
        const name = family === "ge-proton" ? "Proton-GE Latest" : "Proton-CachyOS Latest";
        return { name, path: `/preview/proton/${name}` };
      });
      if (new URLSearchParams(location.search).get("protons") === "none") return { value: installed };
      return { value: [{ name: "Proton-GE Latest", path: "/preview/proton/Proton-GE Latest" }, { name: "Proton Experimental", path: "/preview/Proton-Experimental" }, ...installed.filter(item => item.name !== "Proton-GE Latest")] };
    }
    case "scan_nexus_library": await sleep(1200); return { value: clone(orderWorkspaceMods(workspace)) };
    case "load_nexus_workspace": return { value: clone(orderWorkspaceMods(workspace)) };
    case "list_nexus_catalog_games": return { value: [
      ...specs.map(([name, , domain_name]) => ({ name: domain_name === "tombraider" ? "Tomb Raider (2013)" : name, domain_name, vortex_supported: true })),
      { name: "Rise of the Tomb Raider", domain_name: "riseofthetombraider", vortex_supported: true },
      { name: "Tomb Raider Remastered", domain_name: "tombraiderremastered", vortex_supported: true },
    ] };
    case "list_nexus_catalog_mods": {
      await sleep();
      const mods = domain === "tombraider" && !tombRaiderAssociated ? [] : catalog(domain);
      return { value: args.feed === "trending" ? mods.slice(0, 2) : args.feed === "latest_updated" ? mods.slice(1, 3).reverse() : mods.reverse() };
    }
    case "list_nexus_catalog_page": {
      await sleep();
      if (domain === "tombraider" && !tombRaiderAssociated) return {value:{mods:[],total_count:0,next_offset:0}};
      const originals = catalog(domain);
      let mods = Array.from({length:64},(_,i) => ({...originals[i % originals.length],mod_id:i + 1,name:i < 4 ? originals[i].name : `${originals[i % originals.length].name} — exemplo ${i + 1}`,downloads:120000 - i * 1500}));
      const words = String(args.search || "").toLocaleLowerCase().trim().split(/\s+/).filter(Boolean);
      if (words.length) mods = mods.filter(mod => words.every(word => mod.name.toLocaleLowerCase().includes(word)));
      const offset = Number(args.offset) || 0, count = Number(args.count) || 20;
      return {value:{mods:mods.slice(offset,offset+count),total_count:mods.length,next_offset:Math.min(mods.length,offset+count)}};
    }
    case "nexus_deploy_options": return {value:["copy","symlink","hardlink","vfs"].map(method => ({method,available:true,reason:"Demonstração visual"}))};
    case "configure_nexus_domain": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId);
      if (game) {
        game.nexus_domain = String(args.gameDomain);
        if (game.game.app_id === "203160" && args.gameDomain === "tombraider") {
          tombRaiderAssociated = true;
          game.game.name = "Tomb Raider (2013)";
        }
      }
      return {value:clone(orderWorkspaceMods(workspace))};
    }
    case "configure_nexus_settings": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId);
      if (game) { game.game.directory = String(args.directory); game.compat_data = String(args.compatData); game.proton = String(args.proton); }
      return {value:clone(orderWorkspaceMods(workspace))};
    }
    case "get_nexus_launch_options": {
      const game = workspace.games.find(game => game.id === args.gameId);
      const automatic = "";
      const dll_overrides = game?.platform === "proton" ? "winhttp=n,b" : "";
      return {value: {automatic, effective: game?.launch_options ?? automatic, customized: game?.launch_options != null, dll_overrides}};
    }
    case "configure_nexus_advanced": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId);
      if(game) game.advanced = args.settings as NexusGame["advanced"];
      return {value:clone(orderWorkspaceMods(workspace))};
    }
    case "configure_nexus_launch_options": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId);
      if (game) game.launch_options = args.options == null ? null : String(args.options);
      return {value: clone(orderWorkspaceMods(workspace))};
    }
    case "configure_nexus_deploy": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId);
      if(game)game.deploy_method = args.method as NexusGame["deploy_method"];
      return {value:clone(orderWorkspaceMods(workspace))};
    }
    case "translate_nexus_description": await sleep(); return { value: "Tradução simulada para português Brasil.\n\nNesta prévia, nenhuma descrição é enviada ao serviço de tradução. No programa, este botão traduz o texto original do autor e permite voltar ao original." };
    case "get_nexus_catalog_mod": await sleep(); return { value: details(domain, Number(args.modId)) };
    case "list_nexus_modules": return { value: { definitions: specs.map(([, , domain]) => ({ domain, tools: domain === "newvegas" ? [{name:"FNVEdit"},{name:"Wrye Bash"},{name:"FNV 4GB Patcher"}] : [], activation: domain === "baldursgate3" ? { kind: "baldurs_gate3" } : null })), errors: [] } };
    case "open_nexus_game_tool": throw new Error(`Ferramenta não instalada (demonstração): ${String(args.name)}.`); // Never launches an external tool.
    case "nexus_load_order": return { value: clone(demoLoadOrder) };
    case "set_nexus_load_order": {
      const order = args.order as string[];
      demoLoadOrder.entries.sort((a, b) => order.indexOf(a.file) - order.indexOf(b.file));
      return { value: clone(demoLoadOrder) };
    }
    case "set_nexus_load_order_auto_export":
      demoLoadOrder.auto_export = Boolean(args.enabled);
      return { value: clone(demoLoadOrder) };
    case "import_nexus_load_order":
    case "export_nexus_load_order": return { value: clone(demoLoadOrder) };
    case "inspect_nexus_game": {
      const game = workspace.games.find(g => g.game.directory === args.source) || workspace.games[0];
      return { value: { domain: game.adapter, name: game.game.name, module_version: "Prévia", notes: ["Configurações simuladas para visualizar a interface."], mod_destinations: ["Mods"], isolated_profile: false, frameworks: [] } };
    }
    case "launch_nexus_game":
    case "stop_nexus_game": {
      const game = workspace.games.find(g => g.id === args.gameId);
      if (game) game.running = command === "launch_nexus_game";
      return { value: clone(orderWorkspaceMods(workspace)) };
    }
    case "open_folder":
    case "open_nexus_game_tool": return { value: null };
    case "plugin:dialog|ask": return { value: true };
    case "plugin:dialog|message": return { value: "Yes" };
    case "load_nexus_account": return { value: { user_id: 0, name: "Demonstração visual", is_premium: false, is_supporter: false } };
    case "register_nexus_handler": return { value: null };
    case "nexus_report_relay_configured": return { value: true };
    case "send_nexus_report": throw new Error("Prévia visual: nenhum relatório é enviado ao Discord.");
    case "export_nexus_report": {
      const game = workspace.games.find(g => g.id === args.gameId) || workspace.games[0];
      const content = `PELIGAMES — RELATÓRIO NEXUS (DEMONSTRAÇÃO)\nJogo: ${game.game.name}\nLauncher: ${args.buildLabel}\nSistema: Linux — demonstração\nProton: ${game.proton}\n\nMODS\n${game.mods.map(m => `${m.name} | ativo=${m.enabled}`).join("\n")}\n\n[INFO] Instalação concluída.\n[INFO] Nenhum arquivo real foi gerado nesta prévia.`;
      return {value:{path:`/preview/Logs/Nexus/${game.id}/erro/relatorio-nexus.txt`,content,bytes:content.length,truncated:false}};
    }
    case "refresh_nexus_mod_labels": return { value: clone(workspace.games.find(game => game.id === args.gameId)?.mods || []) };
    case "list_nexus_downloads": return { value: clone(jobs) };
    case "queue_nexus_downloads": {
      await sleep(800);
      for (const file of args.files as { domain: string; mod_id: number; file_id: number }[]) {
        jobs.push({ id: crypto.randomUUID(), ...file, game_id: String(args.gameId), name: `${details(file.domain, file.mod_id).info.name}.zip`, status: "queued", received: 0, total: 8388608, auto_install: false });
      }
      advance(); return { value: null };
    }
    case "preview_confirm_nexus_download": confirm(); return { value: null };
    case "cancel_nexus_download": { const job = jobs.find(j => j.id === args.downloadId); if (job) job.status = "cancelled"; window.setTimeout(advance, 1500); return { value: null }; }
    case "retry_nexus_download": { const job = jobs.find(j => j.id === args.downloadId); if (job) { job.status = "authorizing"; job.error = undefined; } return { value: null }; }
    case "open_nexus_browser": window.dispatchEvent(new CustomEvent("nexus-browser-preview", {detail:"preview"})); return {value:null};
    case "nexus_browser_session": return {value:null};
    case "close_nexus_browser": window.dispatchEvent(new CustomEvent("nexus-browser-preview", {detail:null})); return {value:null};
    case "nexus_browser_action":
    case "resize_nexus_browser": return {value:null};
    case "plugin:clipboard-manager|write_text": await navigator.clipboard.writeText(String(args.text)); return { value: null };
    case "open_browser_url":
    case "plugin:opener|open_url": return { value: null };
    case "load_nexus_collection": {
      await sleep();
      return { value: { domain: "stardewvalley", slug: "htknoa", revision: 1, name: "Coldguy's Stardew Valley Starter Set", mods: [
        { mod_id: 3753, file_id: 44014, name: "Stardew Valley Expanded", file_name: "-Stardew Valley Expanded--3753-1-13-11-1629243294.zip", size: 237905916, version: "1.13.11", optional: false, present: false },
        { mod_id: 2400, file_id: 70000, name: "SMAPI - Stardew Modding API", file_name: "SMAPI 3.18.6-2400-3-18-6-1700000000.zip", size: 8650000, version: "3.18.6", optional: false, present: true },
        { mod_id: 1915, file_id: 50000, name: "Content Patcher", file_name: "Content Patcher-1915-1-30-4-1700000001.zip", size: 410000, version: "1.30.4", optional: false, present: false },
        { mod_id: 2213, file_id: 38709, name: "Unlimited Players", file_name: "2021_02_27_UnlimitedPlayers-2213-2021-2-27-1614446233.zip", size: 5109, version: "2021.2.27", optional: true, present: false },
        { mod_id: 1150, file_id: 40000, name: "Lookup Anything", file_name: "Lookup Anything-541-1-40-0-1700000002.zip", size: 1300000, version: "1.40.0", optional: true, present: false },
      ] } };
    }
    case "install_nexus_collection": { await sleep(); return { value: { queued: 2 + (args.optional as unknown[]).length, present: 1, manifest_error: null } }; }
    case "move_nexus_mod": {
      await sleep();
      const game = workspace.games.find(g => g.id === args.gameId);
      if (game) {
        const i = game.mods.findIndex(m => m.id === args.modId), j = args.up ? i - 1 : i + 1;
        if (i >= 0 && j >= 0 && j < game.mods.length) {
          const before = [...game.mods];
          [game.mods[i], game.mods[j]] = [game.mods[j], game.mods[i]];
          game.mods = orderWorkspaceMods({games:[game]}).games[0].mods;
          if (game.mods.findIndex(m => m.id === args.modId) === i) {
            game.mods = before;
            throw new Error("Os requisitos precisam permanecer antes dos mods que dependem deles.");
          }
        }
      }
      return { value: clone(orderWorkspaceMods(workspace)) };
    }
    case "nexus_mod_overrides": {
      const game = workspace.games.find(g => g.id === args.gameId), active = game?.mods.filter(m => m.installed && m.enabled) || [];
      return { value: active.length > 1 ? [{ target: "Mods/Exemplo/textures/armadura.dds", winner: active[active.length - 1].id, overridden: [active[0].id] }] : [] };
    }
    case "nexus_package_layout": {
      const mod = workspace.games.find(game => game.id === args.gameId)?.mods.find(mod => mod.id === args.modId) as { manual_root?: unknown } | undefined;
      return { value: { folders: ["", "Autor - Pacote v1.0/", "Autor - Pacote v1.0/Opcional/", "Autor - Pacote v1.0/Principal/"], destinations: ["Data", "Mods"], current: mod?.manual_root ?? null } };
    }
    case "set_nexus_manual_root": {
      await sleep();
      const mod = workspace.games.find(game => game.id === args.gameId)?.mods.find(mod => mod.id === args.modId) as { manual_root?: unknown } | undefined;
      if (mod) mod.manual_root = args.root ?? undefined;
      return { value: clone(orderWorkspaceMods(workspace)) };
    }
    case "plan_nexus_installation": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId), mod = game?.mods.find(mod => mod.id === args.modId);
      if (mod?.id === "demo-unstructured") {
        const root = (mod as { manual_root?: { source: string; destination: string } }).manual_root;
        if (!root) return { value: { module: "Módulo de demonstração", files: [], missing: [], notes: [], issues: ["O pacote não contém arquivos reconhecidos pelo módulo do jogo."], selection: [] } };
        const base = root.destination || "Data";
        return { value: { manual_root: root, module: "Módulo de demonstração", files: [[`${root.source}plugin.esp`, `${base}/plugin.esp`], [`${root.source}textures/armadura.dds`, `${base}/textures/armadura.dds`]], missing: [], notes: ["Prévia visual: nenhum arquivo será aplicado ao jogo."], issues: [], selection: [] } };
      }
      const missing = (mod?.nexus_source?.requirements.items || []).filter(item => {
        const modId = Number(item.url?.match(/mods\/(\d+)/)?.[1]);
        return !game?.mods.some(local => local.nexus_source?.mod_id === modId && local.installed && local.enabled);
      }).map(item => ({...item,mod_id:Number(item.url?.match(/mods\/(\d+)/)?.[1])}));
      const others = game?.mods.filter(m => m.id !== mod?.id && m.installed && m.enabled) || [];
      return { value: { overrides: others.length ? [["Mods/Exemplo/textures/armadura.dds", others[0].name]] : [], module: "Módulo de demonstração", files: [["manifest.json", "Mods/Exemplo/manifest.json"]], missing, notes: ["Prévia visual: nenhum arquivo será aplicado ao jogo."], issues: [], selection: [] } };
    }
    case "install_nexus_mod":
    case "set_nexus_mod_enabled":
    case "remove_nexus_archive": {
      if (command === "set_nexus_mod_enabled") {
        for (let done = 0; done <= 200; done += 20) {
          window.dispatchEvent(new CustomEvent("previewNexusDeployProgress", { detail: { game_id: args.gameId, phase: "copy", detail: "Demonstração", done, total: 200 } }));
          await sleep(150);
        }
      } else await sleep();
      const game = workspace.games.find(g => g.id === args.gameId);
      const mod = game?.mods.find(m => m.id === args.modId);
      if (command === "remove_nexus_archive" && game) game.mods = game.mods.filter(m => m.id !== args.modId);
      else if (mod) { mod.installed = true; mod.enabled = command === "install_nexus_mod" || Boolean(args.enabled); }
      return { value: clone(orderWorkspaceMods(workspace)) };
    }
  }
}

// Demonstration-only runner endpoints: no downloads or filesystem writes.
const demoRunnerInstalled = new Set<string>(new URLSearchParams(location.search).get("protonUpdates") === "1" ? ["ge-proton", "cachyos-proton"] : []);
const demoRunnerOutdated = new Set(demoRunnerInstalled);
let cancelDemoRunner: (() => void) | undefined;
export function demoRunnerFetch(input: string): Response | undefined {
  if (!input.startsWith('/__preview/runner/')) return;
  const family = new URL(input, location.origin).searchParams.get('family') ?? 'ge-proton';
  const headers = { 'Content-Type': 'application/json' };
  if (input.startsWith('/__preview/runner/check')) {
    const latest = family === 'ge-proton' ? 'GE-Proton — demonstração' : 'CachyOS — demonstração';
    return new Response(JSON.stringify({ result: { family, latest, current: demoRunnerInstalled.has(family) ? (demoRunnerOutdated.has(family) ? `${latest} anterior` : latest) : null, installed_path: null, needs_update: !demoRunnerInstalled.has(family) || demoRunnerOutdated.has(family), release_url: '', asset: { name: 'pacote-simulado.tar.gz', size: 500 * 1024 * 1024 } } }), { headers });
  }
  if (input.startsWith('/__preview/runner/uninstall')) {
    demoRunnerInstalled.delete(family);
    demoRunnerOutdated.delete(family);
    return new Response('{}', { headers });
  }
  if (input.startsWith('/__preview/runner/cancel')) {
    cancelDemoRunner?.();
    return new Response('{}', { headers });
  }
  if (input.startsWith('/__preview/runner/install')) {
    const encoder = new TextEncoder();
    return new Response(new ReadableStream({ start(controller) {
      let percent = 0;
      const send = (value: unknown) => controller.enqueue(encoder.encode(JSON.stringify(value) + '\n'));
      const timer = window.setInterval(() => {
        percent += 5;
        send({ progress: { family, phase: percent < 100 ? 'downloading' : 'extracting', downloaded: percent * 5 * 1024 * 1024, total: 500 * 1024 * 1024, speed_bytes_per_sec: 2.9 * 1024 * 1024 } });
        if (percent >= 100) { window.clearInterval(timer); cancelDemoRunner = undefined; demoRunnerInstalled.add(family); demoRunnerOutdated.delete(family); send({ result: { path: '/preview/proton/' + (family === 'ge-proton' ? 'Proton-GE Latest' : 'Proton-CachyOS Latest') } }); controller.close(); }
      }, 500);
      cancelDemoRunner = () => { window.clearInterval(timer); cancelDemoRunner = undefined; send({ error: 'Download simulado cancelado.' }); controller.close(); };
    } }), { headers: { 'Content-Type': 'application/x-ndjson' } });
  }
}

// Visual fixtures only: no Nexus account, downloads or game files are accessed.
import type { GameInfo } from "./components/GameGrid";
import type { NexusGame } from "./modules/nexus/useNexusWorkspace";
import type { NexusDownload } from "./modules/nexus/NexusDownloadsPanel";
const specs = [
  ["Stardew Valley", "413150", "stardewvalley"],
  ["Palworld", "1623730", "palworld"],
  ["Cyberpunk 2077", "1091500", "cyberpunk2077"],
  ["Final Fantasy XII The Zodiac Age", "595520", "finalfantasy12"],
];
export const demoGames: GameInfo[] = specs.map(([name, id], i) => ({ name, app_id: id, path: `/preview/game-${i}/game.exe`, directory: `/preview/game-${i}`, launcher: i > 1 ? "PeliGames" : "Steam", cover_url: `https://cdn.cloudflare.steamstatic.com/steam/apps/${id}/library_600x900.jpg` }));
const workspace: { games: NexusGame[] } = { games: demoGames.map((game, i) => ({ id: `demo-${i}`, game, platform: "proton", adapter: specs[i][2], nexus_domain: specs[i][2], running: false, compat_data: `/preview/prefix-${i}`, proton: "/preview/GE-Proton", mods: [
  { id: "demo-installed", name: "Mod visual — exemplo ativo", version: "2.0.0", archive: "/preview/visual.zip", size: 2048000, installed: true, enabled: true },
  { id: "demo-imported", name: "Mod de interface — exemplo importado", version: "1.2.0", archive: "/preview/interface.zip", size: 4096000, installed: false, enabled: false, nexus_source: { domain: specs[i][2], mod_id: 3, file_id: 30, requirements: { complete: true, items: i === 2 ? [{name:"TweakXL",kind:"nexus",notes:"",url:"https://www.nexusmods.com/cyberpunk2077/mods/4197"}] : [] } } },
  ...(i === 2 ? [
    {id:"demo-tweakxl",name:"TweakXL",archive:"/preview/TweakXL.zip",size:1045504,installed:false,enabled:false,nexus_source:{domain:"cyberpunk2077",mod_id:4197,file_id:41970,requirements:{complete:true,items:[{name:"RED4ext",notes:"",kind:"nexus" as const,url:"https://www.nexusmods.com/cyberpunk2077/mods/2380"}]}}},
    {id:"demo-red4ext",name:"RED4ext",archive:"/preview/RED4ext.zip",size:575488,installed:false,enabled:false,nexus_source:{domain:"cyberpunk2077",mod_id:2380,file_id:23800,requirements:{complete:true,items:[]}}},
  ] : []),
] })) };
let jobs: NexusDownload[] = [];
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
  const domain = String(args.gameDomain || "stardewvalley");
  switch (command) {
    case "read_nexus_mod_logs": return {value:`Demonstração — registros simulados\n--- red4ext/logs/red4ext.log | modified_unix=${Math.floor(Date.now()/1000)} ---\n[RED4ext] Plugin TweakXL loaded\n[TweakXL] Loading tweaks: example.yaml`};
    case "read_nexus_logs": return {value: args.previous ? "Demonstração — execução anterior\nJogo encerrado." : "Demonstração — execução atual\nIniciando o jogo com mods."};
    case "scan_installed_protons": return {value:[{name:"GE-Proton",path:"/preview/GE-Proton"},{name:"Proton Experimental",path:"/preview/Proton-Experimental"}]};
    case "scan_nexus_library": await sleep(1200); return { value: clone(workspace) };
    case "load_nexus_workspace": return { value: clone(workspace) };
    case "list_nexus_catalog_games": return { value: specs.map(([name, , domain_name]) => ({ name, domain_name, vortex_supported: true })) };
    case "list_nexus_catalog_mods": {
      await sleep();
      const mods = catalog(domain);
      return { value: args.feed === "trending" ? mods.slice(0, 2) : args.feed === "latest_updated" ? mods.slice(1, 3).reverse() : mods.reverse() };
    }
    case "list_nexus_catalog_page": {
      await sleep();
      const originals = catalog(domain);
      const mods = Array.from({length:64},(_,i) => ({...originals[i % originals.length],mod_id:i + 1,name:i < 4 ? originals[i].name : `${originals[i % originals.length].name} — exemplo ${i + 1}`,downloads:120000 - i * 1500}));
      const offset = Number(args.offset) || 0, count = Number(args.count) || 20;
      return {value:{mods:mods.slice(offset,offset+count),total_count:mods.length,next_offset:Math.min(mods.length,offset+count)}};
    }
    case "nexus_deploy_options": return {value:["copy","symlink","hardlink","vfs"].map(method => ({method,available:true,reason:"Demonstração visual"}))};
    case "configure_nexus_settings": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId);
      if (game) { game.game.directory = String(args.directory); game.compat_data = String(args.compatData); game.proton = String(args.proton); }
      return {value:clone(workspace)};
    }
    case "configure_nexus_deploy": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId);
      if(game)game.deploy_method = args.method as NexusGame["deploy_method"];
      return {value:clone(workspace)};
    }
    case "translate_nexus_description": await sleep(); return { value: "Tradução simulada para português Brasil.\n\nNesta prévia, nenhuma descrição é enviada ao serviço de tradução. No programa, este botão traduz o texto original do autor e permite voltar ao original." };
    case "get_nexus_catalog_mod": await sleep(); return { value: details(domain, Number(args.modId)) };
    case "list_nexus_modules": return { value: { definitions: specs.map(([, , domain]) => ({ domain, tools: [] })), errors: [] } };
    case "inspect_nexus_game": {
      const game = workspace.games.find(g => g.game.directory === args.source) || workspace.games[0];
      return { value: { domain: game.adapter, name: game.game.name, module_version: "Prévia", notes: ["Configurações simuladas para visualizar a interface."], mod_destinations: ["Mods"], isolated_profile: false, frameworks: [] } };
    }
    case "launch_nexus_game":
    case "stop_nexus_game": {
      const game = workspace.games.find(g => g.id === args.gameId);
      if (game) game.running = command === "launch_nexus_game";
      return { value: clone(workspace) };
    }
    case "open_folder":
    case "open_nexus_game_tool": return { value: null };
    case "plugin:dialog|ask": return { value: true };
    case "plugin:dialog|message": return { value: "Yes" };
    case "load_nexus_account": return { value: { user_id: 0, name: "Demonstração visual", is_premium: false, is_supporter: false } };
    case "register_nexus_handler": return { value: null };
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
    case "plan_nexus_installation": {
      await sleep();
      const game = workspace.games.find(game => game.id === args.gameId), mod = game?.mods.find(mod => mod.id === args.modId);
      const missing = (mod?.nexus_source?.requirements.items || []).filter(item => {
        const modId = Number(item.url?.match(/mods\/(\d+)/)?.[1]);
        return !game?.mods.some(local => local.nexus_source?.mod_id === modId && local.installed && local.enabled);
      }).map(item => ({...item,mod_id:Number(item.url?.match(/mods\/(\d+)/)?.[1])}));
      return { value: { module: "Módulo de demonstração", files: [["manifest.json", "Mods/Exemplo/manifest.json"]], missing, notes: ["Prévia visual: nenhum arquivo será aplicado ao jogo."], issues: [], selection: [] } };
    }
    case "install_nexus_mod":
    case "set_nexus_mod_enabled":
    case "remove_nexus_archive": {
      await sleep();
      const game = workspace.games.find(g => g.id === args.gameId);
      const mod = game?.mods.find(m => m.id === args.modId);
      if (command === "remove_nexus_archive" && game) game.mods = game.mods.filter(m => m.id !== args.modId);
      else if (mod) { mod.installed = true; mod.enabled = command === "install_nexus_mod" || Boolean(args.enabled); }
      return { value: clone(workspace) };
    }
  }
}

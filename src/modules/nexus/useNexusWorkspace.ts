import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Requirements } from "./NexusRequirements";
import type { GameInfo } from "../../components/GameGrid";
import { fetchGameCoverByName, hasCustomCover } from "../../services/customCovers";
import { applyCustomCovers } from "../../services/gameLibraryCache";
import { hasNexusCatalog, matchingCatalogGame, type CatalogGame } from "./catalogGameMatch";
import { setLibraryGameRemoved } from "../../services/libraryVisibility";
import { useNexusText } from "./text";
export interface LocalMod { id: string; name: string; version?: string; identification_note?: string; archive: string; size: number; installed: boolean; enabled: boolean; fomod_selection?: string[]; nexus_source?: { name?: string; version?: string; domain: string; mod_id: number; file_id: number; requirements: Requirements } }
export type DeployMethod = "copy" | "symlink" | "hardlink" | "vfs";
export interface NexusGame { deploy_method?: DeployMethod; id: string; game: GameInfo; platform: "native" | "proton"; mods: LocalMod[]; profile?: string; running: boolean; adapter?: string; compat_data?: string; proton?: string; nexus_domain?: string; detected_mods?: { name: string; enabled: boolean; mod_id?: number }[] }
export interface NexusWorkspace { games: NexusGame[] }
export function useNexusWorkspace() {
  const text = useNexusText();
  const [workspace, setWorkspace] = useState<NexusWorkspace>({ games: [] });
  const [busy, setBusy] = useState(false);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [catalogGames, setCatalogGames] = useState<CatalogGame[]>([]);
  const [error, setError] = useState("");
  const lock = useRef(false);
  const mutationRevision = useRef(0);
  const mounted = useRef(true);
  const coverSearches = useRef(new Set<string>());
  const covering = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const run = async (command: string, args?: Record<string, unknown>, silent = false) => {
    if (lock.current) return false;
    lock.current = true;
    mutationRevision.current += 1;
    const setPending = silent ? setSettingsBusy : setBusy;
    setPending(true); setError("");
    try {
      const result = await invoke<NexusWorkspace>(command, args);
      if (command === "scan_nexus_library") {
        try {
          const catalog = await invoke<CatalogGame[]>("list_nexus_catalog_games");
          if (mounted.current) setCatalogGames(catalog);
        } catch (e) {
          // Keep previously confirmed matches on temporary API/account failures.
          if (mounted.current) setError(String(e));
        }
      }
      if (command === "add_nexus_game") {
        const source = args?.game as GameInfo | undefined;
        const entry = result.games.find(item => item.game.path === source?.path);
        if (entry) setLibraryGameRemoved({ ...entry.game, library_view: "nexus" }, false);
      }
      if (mounted.current) setWorkspace(result);
      return true;
    } catch (e) { if (mounted.current) setError(String(e)); return false; }
    finally { lock.current = false; if (mounted.current) { setPending(false); setLoaded(true); } }
  };
  useEffect(() => {
    if (!loaded) return;
    const timer = window.setInterval(() => {
      if (lock.current) return;
      const revision = mutationRevision.current;
      void invoke<NexusWorkspace>("load_nexus_workspace").then(result => {
        // A refresh started before a save must not restore the old settings.
        if (mounted.current && !lock.current && revision === mutationRevision.current) setWorkspace(result);
      }).catch(() => {});
    }, 2500);
    return () => window.clearInterval(timer);
  }, [loaded]);
  const visibleGames = workspace.games.filter(entry => hasNexusCatalog(catalogGames, entry));
  const refreshCover = async (entry: NexusGame) => {
    const key = `${entry.id}:${entry.game.name}:${entry.game.cover_url || ""}`;
    if (hasCustomCover({ ...entry.game, library_view: "nexus" }) || coverSearches.current.has(key)) return;
    coverSearches.current.add(key);
    try {
      const coverUrl = await fetchGameCoverByName(entry.game.name);
      if (!coverUrl || !mounted.current) return;
      const revision = mutationRevision.current;
      const result = await invoke<NexusWorkspace>("set_nexus_game_cover", { gameId: entry.id, name: entry.game.name, coverUrl, previousCover: entry.game.cover_url || null });
      if (!mounted.current) return;
      if (!lock.current && revision === mutationRevision.current) setWorkspace(result);
      const updated = result.games.find(game => game.id === entry.id);
      if (updated) window.dispatchEvent(new CustomEvent("gameCoverChanged", { detail: applyCustomCovers([{ ...updated.game, library_view: "nexus" }])[0] }));
    } catch { /* A missing cover must not interrupt library/mod operations. */ }
  };
  useEffect(() => {
    if (!loaded || busy || settingsBusy || covering.current) return;
    const pending = visibleGames.filter(entry => !entry.game.cover_url && !hasCustomCover({ ...entry.game, library_view: "nexus" }) && !coverSearches.current.has(`${entry.id}:${entry.game.name}:`));
    if (!pending.length) return;
    covering.current = true;
    void (async () => {
      try { for (const entry of pending) { if (!mounted.current) break; await refreshCover(entry); } }
      finally { covering.current = false; }
    })();
  }, [workspace, catalogGames, loaded, busy, settingsBusy]);
  return { workspace, visibleGames, busy, settingsBusy, error, loaded, setError,
    recoverCover: (game: GameInfo) => { const entry = workspace.games.find(entry => entry.game.path === game.path && entry.game.launcher === game.launcher); return entry ? refreshCover(entry) : Promise.resolve(); },
    refresh: () => { coverSearches.current.clear(); return run("scan_nexus_library"); },
    add: async (game: GameInfo, platform: NexusGame["platform"]) => {
      try {
        const report = await invoke<{ domain?: string }>("inspect_nexus_game", { source: game.directory || game.path });
        const catalog = catalogGames.length ? catalogGames : await invoke<CatalogGame[]>("list_nexus_catalog_games");
        if (mounted.current) setCatalogGames(catalog);
        const match = report.domain
          ? catalog.find(item => item.domain_name === report.domain && item.vortex_supported === true)
          : matchingCatalogGame(catalog, game.name);
        if (!match) {
          setError(text.catalogNotMatched);
          return false;
        }
        return run("add_nexus_game", { game, platform });
      } catch (e) { if (mounted.current) setError(String(e)); return false; }
    },
    importArchive: (gameId: string, source: string) => run("import_nexus_archive", { gameId, source }),
    install: (gameId: string, modId: string, selection?: string[]) => run("install_nexus_mod", { gameId, modId, selection: selection ?? null }),
    toggle: (gameId: string, modId: string, enabled: boolean) => run("set_nexus_mod_enabled", { gameId, modId, enabled }),
    configureDomain: (gameId: string, gameDomain: string) => run("configure_nexus_domain", { gameId, gameDomain }),
    configureDeploy: (gameId: string, method: DeployMethod) => run("configure_nexus_deploy", { gameId, method }, true),
    configureProton: (gameId: string, compatData: string, proton: string, directory: string) => run("configure_nexus_settings", { gameId, compatData, proton, directory }, true),
    start: (gameId: string) => run("launch_nexus_game", { gameId }),
    stop: (gameId: string) => run("stop_nexus_game", { gameId }),
    removeArchive: (gameId: string, modId: string) => run("remove_nexus_archive", { gameId, modId }),
  };
}

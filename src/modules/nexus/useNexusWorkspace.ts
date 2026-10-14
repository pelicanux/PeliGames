import { orderWorkspaceMods } from "./dependencyOrder";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Requirements } from "./NexusRequirements";
import type { GameInfo } from "../../components/GameGrid";
import { fetchGameCoverByName, hasCustomCover } from "../../services/customCovers";
import { rememberAutomaticCover } from "../../services/automaticCovers";
import { applyCustomCovers } from "../../services/gameLibraryCache";
import { hasNexusCatalog, matchingCatalogGame, type CatalogGame } from "./catalogGameMatch";
import { setLibraryGameRemoved } from "../../services/libraryVisibility";
import { useNexusText } from "./text";
import { notifyError } from "../../services/notifications";
export interface LocalMod { id: string; name: string; version?: string; identification_note?: string; archive: string; size: number; installed: boolean; enabled: boolean; fomod_selection?: string[]; manual_root?: { source: string; destination: string } | null; nexus_source?: { name?: string; version?: string; domain: string; mod_id: number; file_id: number; requirements: Requirements } }
export type DeployMethod = "copy" | "symlink" | "hardlink" | "vfs";
export interface NexusGame { advanced?: import("../../services/advancedSettings").AdvancedSettings; launch_options?: string | null; deploy_method?: DeployMethod; id: string; game: GameInfo; platform: "native" | "proton"; mods: LocalMod[]; profile?: string; running: boolean; adapter?: string; compat_data?: string; proton?: string; nexus_domain?: string; detected_mods?: { name: string; enabled: boolean; mod_id?: number }[] }
export interface NexusWorkspace { games: NexusGame[] }
/** Deployment progress reported by the backend: extracting into the profile, then placing in the game. */
export interface DeployProgress { phase: "extract" | "copy" | "link"; detail: string; done: number; total: number }
export function useNexusWorkspace() {
  const text = useNexusText();
  const [workspace, setWorkspace] = useState<NexusWorkspace>({ games: [] });
  const [busy, setBusy] = useState(false);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [catalogGames, setCatalogGames] = useState<CatalogGame[]>([]);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState<Record<string, DeployProgress>>({});
  // Preparing the selected game until the launch command returns.
  const [launching, setLaunching] = useState<string | null>(null);
  const [stopping, setStopping] = useState<string | null>(null);
  const stopRequested = useRef<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const lock = useRef(false);
  const scanTask = useRef<Promise<boolean> | null>(null);
  const mutationRevision = useRef(0);
  const mounted = useRef(true);
  const coverSearches = useRef(new Set<string>());
  const covering = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  // A background recovery copy failed (e.g. the game was open): bell, and refresh the backup panels.
  useEffect(() => {
    const stop = listen<{ game: string; message: string }>("mod-backup-error", ({ payload }) => {
      notifyError("Backup Nexus", `${payload.game}: ${payload.message}`);
      window.dispatchEvent(new Event("modBackupSettingsChanged"));
    });
    return () => { void stop.then(unlisten => unlisten()); };
  }, []);
  useEffect(() => {
    const receive = (payload: DeployProgress & { game_id: string }) => {
      if (!mounted.current) return;
      setProgress(current => {
        const next = { ...current };
        if (payload.phase) next[payload.game_id] = { phase: payload.phase as DeployProgress["phase"], detail: payload.detail, done: payload.done, total: payload.total };
        else delete next[payload.game_id];
        return next;
      });
    };
    const stop = listen<DeployProgress & { game_id: string }>("nexus-deploy-progress", ({ payload }) => receive(payload));
    const previewProgress = (event: Event) => receive((event as CustomEvent<DeployProgress & { game_id: string }>).detail);
    if (import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window) window.addEventListener("previewNexusDeployProgress", previewProgress);
    return () => { window.removeEventListener("previewNexusDeployProgress", previewProgress); void stop.then(unlisten => unlisten()); };
  }, []);
  const run = async (command: string, args?: Record<string, unknown>, silent = false) => {
    // Never ignore a click silently: tell the user another action is still running.
    if (lock.current) { if (mounted.current && !silent) setError(text.waitPreviousAction); return false; }
    lock.current = true;
    mutationRevision.current += 1;
    const setPending = silent ? setSettingsBusy : setBusy;
    setPending(true); setError("");
    try {
      const result = await invoke<NexusWorkspace>(command, args);
      if (command === "add_nexus_game") {
        const source = args?.game as GameInfo | undefined;
        const entry = result.games.find(item => item.game.path === source?.path);
        if (entry) setLibraryGameRemoved({ ...entry.game, library_view: "nexus" }, false);
      }
      if (mounted.current) setWorkspace(orderWorkspaceMods(result));
      return true;
    } catch (e) { if (mounted.current) setError(String(e)); return false; }
    finally {
      lock.current = false;
      if (mounted.current) {
        setPending(false); setLoaded(true); setProgress({});
        setError(current => current === text.waitPreviousAction ? "" : current);
      }
    }
  };
  useEffect(() => {
    if (!loaded) return;
    let timer: number | undefined, stopped = false, last = "";
    // Every 2.5 s while the launcher is visible, every 10 s while it is hidden
    // (minimized during a game). Only a real change re-renders the launcher.
    const refresh = () => {
      window.clearTimeout(timer);
      if (stopped) return;
      const next = () => { if (!stopped) timer = window.setTimeout(refresh, document.hidden ? 10000 : 2500); };
      if (lock.current) { next(); return; }
      const revision = mutationRevision.current;
      void invoke<NexusWorkspace>("load_nexus_workspace").then(result => {
        // A refresh started before a save must not restore the old settings.
        if (!mounted.current || lock.current || revision !== mutationRevision.current) return;
        const snapshot = JSON.stringify(result);
        if (snapshot !== last) { last = snapshot; setWorkspace(orderWorkspaceMods(result)); }
      }).catch(() => {}).finally(next);
    };
    const visible = () => { if (!document.hidden) refresh(); };
    timer = window.setTimeout(refresh, 2500);
    document.addEventListener("visibilitychange", visible);
    return () => { stopped = true; window.clearTimeout(timer); document.removeEventListener("visibilitychange", visible); };
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
      if (!lock.current && revision === mutationRevision.current) setWorkspace(orderWorkspaceMods(result));
      const updated = result.games.find(game => game.id === entry.id);
      if (updated && updated.game.cover_url === coverUrl) {
        await rememberAutomaticCover(updated.game, coverUrl);
        if (mounted.current) window.dispatchEvent(new CustomEvent("gameCoverChanged", { detail: applyCustomCovers([{ ...updated.game, library_view: "nexus" }])[0] }));
      }
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
  return { workspace, visibleGames, busy, settingsBusy, error, loaded, setError, progress, launching, stopping, scanning,
    recoverCover: (game: GameInfo) => { const entry = workspace.games.find(entry => entry.game.path === game.path && entry.game.launcher === game.launcher); return entry ? refreshCover(entry) : Promise.resolve(); },
    refresh: () => {
      if (scanTask.current) return scanTask.current;
      coverSearches.current.clear();
      setScanning(true);
      const revision = mutationRevision.current;
      // Discovery has its own pending state: it never locks game interaction.
      const task = (async () => {
        try {
          const result = await invoke<NexusWorkspace>("scan_nexus_library");
          // A concurrent edit owns newer state. Polling will load the merged
          // backend workspace after it finishes, without restoring stale mods.
          if (mounted.current && !lock.current && revision === mutationRevision.current) {
            setWorkspace(orderWorkspaceMods(result));
          }
          try {
            const catalog = await invoke<CatalogGame[]>("list_nexus_catalog_games");
            if (mounted.current) setCatalogGames(catalog);
          } catch (e) { if (mounted.current) setError(String(e)); }
          return true;
        } catch (e) { if (mounted.current) setError(String(e)); return false; }
        finally {
          scanTask.current = null;
          if (mounted.current) { setScanning(false); setLoaded(true); }
        }
      })();
      scanTask.current = task;
      return task;
    },
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
    move: (gameId: string, modId: string, up: boolean) => run("move_nexus_mod", { gameId, modId, up }),
    switchProfile: (gameId: string, id: string) => run("switch_nexus_user_profile", { gameId, id }),
    downloadLoader: (gameId: string, name?: string) => run("download_nexus_loader", { gameId, name: name ?? null }),
    configureAdvanced: (gameId: string, settings: import("../../services/advancedSettings").AdvancedSettings) => run("configure_nexus_advanced", {gameId,settings},true),
    configureLaunch: (gameId: string, options: string | null) => run("configure_nexus_launch_options", { gameId, options }, true),
    configureDomain: (gameId: string, gameDomain: string) => run("configure_nexus_domain", { gameId, gameDomain }),
    configureDeploy: (gameId: string, method: DeployMethod) => run("configure_nexus_deploy", { gameId, method }, true),
    configureProton: (gameId: string, compatData: string, proton: string, directory: string) => run("configure_nexus_settings", { gameId, compatData, proton, directory }, true),
    start: async (gameId: string) => {
      if (lock.current) return run("launch_nexus_game", { gameId });
      stopRequested.current = null; setLaunching(gameId);
      try {
        const started = await run("launch_nexus_game", { gameId });
        if (started && stopRequested.current === gameId) await run("stop_nexus_game", { gameId });
        return started;
      } finally { stopRequested.current = null; if (mounted.current) { setLaunching(null); setStopping(null); } }
    },
    stop: async (gameId: string) => {
      if (launching === gameId) { stopRequested.current = gameId; setStopping(gameId); return true; }
      if (lock.current) return run("stop_nexus_game", { gameId });
      setStopping(gameId);
      try { return await run("stop_nexus_game", { gameId }); } finally { if (mounted.current) setStopping(null); }
    },
    removeArchive: (gameId: string, modId: string) => run("remove_nexus_archive", { gameId, modId }),
    removeGame: (gameId: string) => run("remove_nexus_game", { gameId }),
  };
}

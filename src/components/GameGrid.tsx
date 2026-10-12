import { useAccentTheme } from "../theme/ThemeProvider";
import { createPortal } from "react-dom";
import { CoverWalkingPenguin } from "./CoverWalkingPenguin";
import { CoverHoldParticles } from "./CoverHoldParticles";
import { CoverWaterFill } from "./CoverWaterFill";
import { CoverLaunchTeleport, coverCrackPaths, type CoverTeleport } from "./CoverLaunchTeleport";
import { GamePlatformBadge } from "./GamePlatformBadge";
import { NexusLibraryLoading } from "../modules/nexus/NexusLibraryLoading";
import { useFavorites } from "../services/favorites";
import type { CoverGameAction } from "./CoverGameActions";
import { GameCoverMenu } from "./GameCoverMenu";
import { useRemovedLibraryGames, restoreRemovedLibraryFolder } from "../services/libraryVisibility";
import { gameViewKey } from "../services/gameIdentity";
import { TimedFeedback } from "./TimedFeedback";
import { MenuIcon } from "./MenuIcon";
import { STEAMGRIDDB_API_KEY } from "../services/coverConfig";
import { hasCustomCover } from "../services/customCovers";
import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n/I18nContext";
import { AnimatePresence, motion } from "framer-motion";
import { usePerformanceMode } from "./EffectsContext";
import { applyCustomCovers, readGameLibrary, saveGameLibrary } from "../services/gameLibraryCache";
import { warmAmbientCover } from "../services/ambientCoverCache";
import { GameEntryButton, GameModeSelector, type GamePanelMode } from "./GameModeSelector";
import { HoverTooltip } from "./HoverTooltip";
import { LauncherIcon } from "./LauncherIcon";
import { openDirectoryPicker } from "../services/tauriService";
import { listPeliGamesEntries } from "../services/installedLibrary";

export interface GameInfo {
  nexus_platform?: "native" | "proton";
  nexus_running?: boolean;
  /** ID of the Nexus library entry (several entries may share a folder). */
  nexus_id?: string;
  discovery_source?: string | null;
  /** The clear favourite among an installation's executables. */
  recommended?: boolean;
  launch_arguments?: string[];
  library_view?: "own" | "all" | "nexus";
  advanced?: import("../services/advancedSettings").AdvancedSettings;
  name: string;
  path: string;
  app_id?: string;
  cover_url?: string;
  automatic_cover_url?: string | null;
  launcher: string;
  directory?: string;
  executable?: string;
  prefix?: string;
  proton?: string;
}

export interface ManagedGameLibrary {
  games: GameInfo[]; busy: boolean; loaded: boolean; error?: string; title: string; empty: string; hint: string; addLabel: string;
  /** A library scan is running (also after the first load). */
  scanning?: boolean;
  onAdd: () => void; onRefresh: () => void;
  downloadsAction: React.ReactNode; accountLabel: string; onAccount: () => void;
  onCoverFallback?: (game: GameInfo) => Promise<void>;
}
interface Props {
  managedLibrary?: ManagedGameLibrary;
  onCoverAction: (action: CoverGameAction, game: GameInfo) => void | Promise<unknown>;
  coverActionsBlocked?: boolean;
  coverHoldBlocked?: boolean;
  homeRevision: number;
  onHome: () => void;
  onSelectGame: (game: GameInfo, portrait?: HTMLElement) => void;
  selectedGameKey?: string;
  runningGamePath?: string;
  mode: GamePanelMode;
  detailsOpen: boolean;
  libraryScope: "own" | "all";
  modeDisabled?: boolean;
  onModeChange: (mode: GamePanelMode) => void;
}

export const GameGrid: React.FC<Props> = ({ onCoverAction, coverActionsBlocked, coverHoldBlocked, homeRevision, onHome, onSelectGame, selectedGameKey, runningGamePath, mode, libraryScope, detailsOpen, onModeChange, modeDisabled, managedLibrary }) => {
  const { launchEffect } = useAccentTheme();
  const [, setCoverRevision] = useState(0);
  const managed = Boolean(managedLibrary);
  useEffect(() => {
    if (!managed) return;
    const refresh = (event: Event) => {
      const game = (event as CustomEvent<GameInfo>).detail;
      if (game) setImageErrors(previous => { const next = new Set(previous); next.delete(game.path); return next; });
      setCoverRevision(value => value + 1);
    };
    window.addEventListener("gameCoverChanged", refresh);
    return () => window.removeEventListener("gameCoverChanged", refresh);
  }, [managed]);
  const favorites = useFavorites();
  const removedGames = useRemovedLibraryGames();
  const [favoritesCollapsed, setFavoritesCollapsed] = useState(false);
  const performanceMode = usePerformanceMode();
  const [cachedGames] = useState(() => readGameLibrary());
  const [initialSnapshots] = useState(() => ({
    all: cachedGames,
    own: readGameLibrary("own") ?? cachedGames?.filter(game => game.launcher === "PeliGames") ?? null,
    nexus: readGameLibrary("nexus"),
  }));
  const librarySnapshots = useRef(initialSnapshots);
  const [games, setGames] = useState<GameInfo[]>(cachedGames ?? []);
  const [libraryLoaded, setLibraryLoaded] = useState(cachedGames !== null);
  const startupHandled = useRef(false);
  const scanInProgress = useRef(false);
  const pendingScanFolders = useRef<{ folders: string[]; background: boolean } | null>(null);
  const ownEntries = useRef<GameInfo[]>([]);
  const ownRequest = useRef(0);
  const [ownLoaded, setOwnLoaded] = useState(false);
  const [libraryError, setLibraryError] = useState("");
  const refreshOwnLibrary = async (rescan = false) => {
    const request = ++ownRequest.current;
    try {
      const entries = await listPeliGamesEntries(rescan);
      if (request !== ownRequest.current) return;
      ownEntries.current = entries;
      setLibraryError("");
      setGames(previous => applyCustomCovers([...previous.filter(game => game.launcher !== "PeliGames"), ...entries]));
    } catch (error) { if (request === ownRequest.current) setLibraryError(String(error)); }
  };
  useEffect(() => {
    if (managedLibrary) return;
    void refreshOwnLibrary(true).finally(() => setOwnLoaded(true));
    const refresh = () => { void fetchGames(); };
    window.addEventListener("peligamesLibraryChanged", refresh);
    return () => { ++ownRequest.current; window.removeEventListener("peligamesLibraryChanged", refresh); };
  }, []);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchExpanded, setSearchExpanded] = useState(false);
  const searchInput = useRef<HTMLInputElement>(null);
  const searchButton = useRef<HTMLButtonElement>(null);
  const searchId = React.useId();
  useEffect(() => {
    if (searchExpanded) searchInput.current?.focus();
  }, [searchExpanded]);
  const [viewMode, setViewMode] = useState<"grid" | "list">(() =>
    localStorage.getItem("game_library_view") === "list" ? "list" : "grid"
  );
  useEffect(() => {
    localStorage.setItem("game_library_view", viewMode);
  }, [viewMode]);
  const [loading, setLoading] = useState(false);
  const [imageErrors, setImageErrors] = useState<Set<string>>(new Set());
  const [customFolders, setCustomFolders] = useState<string[]>(() => {
    const saved = localStorage.getItem("custom_folders");
    return saved ? JSON.parse(saved) : [];
  });
  const [collapsedCats, setCollapsedCats] = useState<Record<string, boolean>>({});
  useEffect(() => { setCollapsedCats({}); }, [homeRevision]);
  const [contextMenu, setContextMenu] = useState<{ x: number, y: number, game: GameInfo } | null>(null);
  const { t } = useI18n();
  useEffect(() => { setSearchQuery(""); setSearchExpanded(false); setContextMenu(null); }, [libraryScope, homeRevision]);

  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    document.addEventListener("click", handleClick);
    return () => document.removeEventListener("click", handleClick);
  }, []);

  const [fallbackAttempted, setFallbackAttempted] = useState<Set<string>>(new Set());

  const fetchGames = async (folders: string[] = customFolders, background = false) => {
    if (managedLibrary) { managedLibrary.onRefresh(); return; }
    if (scanInProgress.current) {
      // A focus refresh must not downgrade a pending explicit scan.
      if (!pendingScanFolders.current || !background) pendingScanFolders.current = { folders, background };
      return;
    }
    scanInProgress.current = true;
    // Restoring the window refreshes data without unmounting the visible cards.
    if (!background) {
      setLoading(true);
      setImageErrors(new Set());
      setFallbackAttempted(new Set());
    }
    try {
      if (libraryScope === "own") {
        await refreshOwnLibrary(true);
        setLibraryLoaded(true);
        return;
      }
      const result = await invoke<GameInfo[]>("scan_installed_games", { apiKey: STEAMGRIDDB_API_KEY, customFolders: folders });
      setGames(applyCustomCovers([...result.filter(game => game.launcher !== "PeliGames"), ...ownEntries.current]));
      await refreshOwnLibrary(true);
      setLibraryLoaded(true);
    } catch (e) {
      console.error("Failed to scan games", e);
    } finally {
      scanInProgress.current = false;
      const pending = pendingScanFolders.current;
      pendingScanFolders.current = null;
      if (pending) void fetchGames(pending.folders, pending.background);
      else setLoading(false);
    }
  };

  useEffect(() => {
    if (managed) return;
    const refreshOnFocus = () => { void fetchGames(customFolders, true); };
    window.addEventListener("focus", refreshOnFocus);
    return () => window.removeEventListener("focus", refreshOnFocus);
  }, [managed, customFolders, libraryScope]);

  const handleAddCustomFolder = async () => {
    const dir = await openDirectoryPicker(t("gameGrid", "selectFolder"));
    if (dir) {
      const newFolders = customFolders.includes(dir) ? customFolders : [...customFolders, dir];
      restoreRemovedLibraryFolder(dir);
      setCustomFolders(newFolders);
      localStorage.setItem("custom_folders", JSON.stringify(newFolders));
      await fetchGames(newFolders);
    }
  };

  useEffect(() => {
    if (managedLibrary) return;
    if (libraryScope === "all" && !startupHandled.current) {
      startupHandled.current = true;
      if (cachedGames === null) void fetchGames();
      else void invoke("log_cached_library", { count: cachedGames.length }).catch(() => {});
    }

    const handleRefresh = (event: Event) => {
      const saved = localStorage.getItem("custom_folders");
      const folders: string[] = saved ? JSON.parse(saved) : [];
      const removedFolders = customFolders.filter(folder => !folders.includes(folder));
      const pathChange = (event as CustomEvent<{ oldPath: string; newPath: string }>).detail;
      setCustomFolders(folders);
      setGames(previous => applyCustomCovers(previous
        .filter(game => game.launcher !== "Manual" || !removedFolders.some(folder => game.path === folder || game.path.startsWith(`${folder.replace(/[\\/]$/, "")}/`)))
        .map(game => pathChange && game.path === pathChange.oldPath ? { ...game, path: pathChange.newPath } : game)));
    };
    const handleCoverChange = (event: Event) => {
      const game = (event as CustomEvent<GameInfo>).detail;
      setGames(previous => previous.map(item => gameViewKey({ ...item, library_view: libraryScope }) === gameViewKey(game) ? { ...item, cover_url: game.cover_url, automatic_cover_url: game.automatic_cover_url } : item));
      setImageErrors(previous => { const next = new Set(previous); next.delete(game.path); return next; });
      setFallbackAttempted(previous => { const next = new Set(previous); next.delete(game.path); return next; });
    };
    window.addEventListener("gameCoverChanged", handleCoverChange);
    const handleRescan = () => { void fetchGames(); };
    window.addEventListener("refreshGames", handleRefresh);
    window.addEventListener("rescanGames", handleRescan);
    return () => {
      window.removeEventListener("gameCoverChanged", handleCoverChange);
      window.removeEventListener("refreshGames", handleRefresh);
      window.removeEventListener("rescanGames", handleRescan);
    };
  }, [customFolders, libraryScope]);

  const hoverFrame = useRef<number | null>(null);
  const hoverBounds = useRef<{ card: HTMLDivElement; rect: DOMRect } | null>(null);
  const hoverPosition = useRef({ x: 0, y: 0 });
  useEffect(() => () => {
    if (hoverFrame.current !== null) cancelAnimationFrame(hoverFrame.current);
    hoverFrame.current = null;
    hoverBounds.current = null;
  }, [performanceMode, viewMode]);


  const handleMouseEnter = (event: React.MouseEvent<HTMLDivElement>) => {
    if (viewMode === "list" || performanceMode) return;
    hoverBounds.current = { card: event.currentTarget, rect: event.currentTarget.getBoundingClientRect() };
  };
  const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
    if (viewMode === "list" || performanceMode) return;
    const card = event.currentTarget;
    if (hoverBounds.current?.card !== card) handleMouseEnter(event);
    hoverPosition.current = { x: event.clientX, y: event.clientY };
    if (hoverFrame.current !== null) return;
    // Follow the display refresh rate, with one DOM update per rendered frame.
    hoverFrame.current = requestAnimationFrame(() => {
      hoverFrame.current = null;
      const bounds = hoverBounds.current;
      if (!bounds || bounds.card !== card) return;
      const { rect } = bounds;
      const x = hoverPosition.current.x - rect.left, y = hoverPosition.current.y - rect.top;
      const rotateX = ((y - rect.height / 2) / (rect.height / 2)) * -15;
      const rotateY = ((x - rect.width / 2) / (rect.width / 2)) * 15;
      card.style.transform = `perspective(1000px) rotateX(${rotateX}deg) rotateY(${rotateY}deg) scale3d(1.05, 1.05, 1.05)`;
      card.style.zIndex = "20";
      card.style.boxShadow = "0 0 20px rgba(var(--accent-rgb), 0.6), 0 15px 30px rgba(0,0,0,0.6)";
      card.style.borderColor = "rgba(var(--accent-rgb), 0.8)";
      const glare = card.querySelector('.glare') as HTMLDivElement;
      if (glare) {
        glare.style.background = `radial-gradient(circle at ${x}px ${y}px, rgba(255, 255, 255, 0.4) 0%, transparent 60%)`;
        glare.style.opacity = "1";
      }
    });
  };

  const handleMouseLeave = (e: React.MouseEvent<HTMLDivElement>) => {
    const card = e.currentTarget;
    if (hoverFrame.current !== null) cancelAnimationFrame(hoverFrame.current);
    hoverFrame.current = null;
    hoverBounds.current = null;
    card.style.transform = (viewMode === "list" || performanceMode) ? "none" : `perspective(1000px) rotateX(0deg) rotateY(0deg) scale3d(1, 1, 1)`;
    card.style.zIndex = "1";
    card.style.boxShadow = "none";
    card.style.borderColor = "rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)";
    
    const glare = card.querySelector('.glare') as HTMLDivElement;
    if (glare) {
      glare.style.opacity = "0";
    }
  };

  const [holdingKey, setHoldingKey] = useState<string | null>(null);
  const [brokenCards, setBrokenCards] = useState(() => new Set<string>());
  const [teleport, setTeleport] = useState<CoverTeleport | null>(null);
  const [launchCommitted, setLaunchCommitted] = useState(false);
  const cancelPendingLaunch = useRef<(() => void) | null>(null);
  const launchGeneration = useRef(0);
  useEffect(() => () => { ++launchGeneration.current; cancelPendingLaunch.current?.(); }, []);
  const hold = useRef<{ key: string; pointer: number; x: number; y: number; timer: number; feedbackTimer?: number; target: HTMLDivElement } | null>(null);
  const suppressHoldClick = useRef<string | null>(null);
  const launchBlocked = useRef(coverHoldBlocked ?? coverActionsBlocked);
  launchBlocked.current = coverHoldBlocked ?? coverActionsBlocked;
  const runningSnapshot = useRef({ path: runningGamePath, games: managedLibrary?.games });
  runningSnapshot.current = { path: runningGamePath, games: managedLibrary?.games };
  const isRunning = (game: GameInfo) => game.path === runningSnapshot.current.path || Boolean(runningSnapshot.current.games?.find(entry => entry.path === game.path && entry.launcher === game.launcher)?.nexus_running);
  const launchFromHold = useRef(onCoverAction);
  launchFromHold.current = onCoverAction;
  const cancelHold = () => {
    const active = hold.current;
    hold.current = null;
    if (active) {
      window.clearTimeout(active.timer);
      window.clearTimeout(active.feedbackTimer);
      if (active.target.hasPointerCapture(active.pointer)) active.target.releasePointerCapture(active.pointer);
    }
    setHoldingKey(null);
  };
  useEffect(() => {
    const cancel = () => cancelHold();
    const visibility = () => { if (document.hidden) cancel(); };
    window.addEventListener("pointerup", cancel);
    window.addEventListener("pointercancel", cancel);
    window.addEventListener("blur", cancel);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      cancelHold();
      window.removeEventListener("pointerup", cancel);
      window.removeEventListener("pointercancel", cancel);
      window.removeEventListener("blur", cancel);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, []);
  useEffect(() => { cancelHold(); }, [coverHoldBlocked, coverActionsBlocked, libraryScope, viewMode, mode, launchEffect]);
  useEffect(() => {
    cancelPendingLaunch.current?.();
    ++launchGeneration.current;
    setTeleport(null);
    setBrokenCards(new Set());
  }, [homeRevision, libraryScope, mode]);
  const startHold = (event: React.PointerEvent<HTMLDivElement>, game: GameInfo) => {
    cancelHold();
    suppressHoldClick.current = null;
    if (launchEffect === "disabled" || event.button !== 0 || !event.isPrimary || launchBlocked.current
      || !(managed || game.launcher === "Steam" || game.launcher === "PeliGames")) return;
    const key = gameViewKey(game);
    const stopping = isRunning(game);
    if (!stopping && coverActionsBlocked) return;
    const source = (event.currentTarget.closest(".library-card-cell") ?? event.currentTarget).getBoundingClientRect();
    // A lightweight DOM copy keeps the frame, badges and image together; no
    // screenshot encoding or game data processing runs during the gesture.
    const preview = !stopping ? event.currentTarget.cloneNode(true) as HTMLElement : null;
    preview?.classList.remove("is-holding-launch", "is-shattering-launch", "is-holding-particles", "is-holding-shrink");
    preview?.querySelectorAll(".library-cover-shatter, .library-hold-launch").forEach(node => node.remove());
    const timer = window.setTimeout(() => {
      if (hold.current?.key !== key || launchBlocked.current) { cancelHold(); return; }
      suppressHoldClick.current = key;
      cancelHold();
      if (stopping !== isRunning(game)) return;
      if (stopping) {
        void Promise.resolve().then(() => launchFromHold.current("stop", game)).catch(error => console.error("Cover stop failed", error));
        return;
      }
      if (!preview) {
        void Promise.resolve().then(() => launchFromHold.current("start", game)).catch(error => console.error("Cover launch failed", error));
        return;
      }
      setBrokenCards(previous => new Set(previous).add(key));
      const generation = ++launchGeneration.current;
      setLaunchCommitted(false);
      setTeleport({ game, source, preview, effect: launchEffect === "shrink" ? "shrink" : launchEffect === "penguin" ? "penguin" : launchEffect === "particles" ? "particles" : launchEffect === "aquarium" ? "aquarium" : launchEffect === "smooth" ? "smooth" : "shatter" });
      // Give the enlarged cover a cancellation window before launching, including
      // Steam games whose external startup cannot be reliably cancelled afterwards.
      const ready = new Promise<boolean>(resolve => {
        const timer = window.setTimeout(() => {
          cancelPendingLaunch.current = null;
          resolve(true);
        }, (launchEffect === "smooth" || launchEffect === "penguin" || launchEffect === "shrink") ? 3500 : 4050);
        cancelPendingLaunch.current = () => {
          window.clearTimeout(timer);
          cancelPendingLaunch.current = null;
          resolve(false);
        };
      });
      void ready.then(async proceed => {
        if (!proceed || generation !== launchGeneration.current) return;
        setLaunchCommitted(true);
        await launchFromHold.current("start", game);
      }).catch(error => console.error("Cover launch failed", error)).finally(() => {
        if (generation !== launchGeneration.current) return;
        setTeleport(null);
        setBrokenCards(previous => { const next = new Set(previous); next.delete(key); return next; });
      });
    }, 1200);
    hold.current = { key, pointer: event.pointerId, x: event.clientX, y: event.clientY, timer, target: event.currentTarget };
    if (launchEffect === "shrink") {
      hold.current.feedbackTimer = window.setTimeout(() => {
        const active = hold.current;
        if (!active || active.key !== key) return;
        // Shrinking must not make a stationary pointer leave the shrinking hit area.
        active.target.setPointerCapture(active.pointer);
        setHoldingKey(key);
      }, 150);
    } else setHoldingKey(key);
  };

  const runningBadge = (game: GameInfo) => (game.nexus_running || game.path === runningGamePath) && <span className="library-running-badge" role="status"><i aria-hidden="true" />{t("gameExecution", "running")}</span>;
  const renderCardContent = (game: GameInfo) => {
    if (brokenCards.has(gameViewKey(game))) return null;
    if (gameViewKey(game) === selectedGameKey) {
      return (
        <div 
          key={gameViewKey(game)}
          className={`game-card-placeholder ${viewMode === "list" ? "game-card-placeholder--list" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "shatter" ? "is-holding-launch" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "particles" ? "is-holding-particles" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "shrink" ? "is-holding-shrink" : ""}`}
          onPointerDown={event => { if (isRunning(game)) startHold(event, game); }}
          onPointerMove={event => { const active = hold.current; if (active && Math.hypot(event.clientX - active.x, event.clientY - active.y) > 10) cancelHold(); }}
          onPointerLeave={cancelHold}
          onDragStart={event => event.preventDefault()}
          style={{
            background: "rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)",
            border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.12)",
            borderRadius: "8px",
            aspectRatio: "2/3",
            width: "100%",
            opacity: 0.5
          }}
        >
          {holdingKey === gameViewKey(game) && launchEffect === "smooth" && <div className="library-hold-progress" aria-hidden="true"><span /></div>}
          {holdingKey === gameViewKey(game) && launchEffect === "aquarium" && <CoverWaterFill reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "particles" && <CoverHoldParticles reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "penguin" && <CoverWalkingPenguin reducedMotion={performanceMode} />}
          {game.cover_url && !imageErrors.has(game.path) ? (
            <img className="selected-cover-copy" src={game.cover_url} alt="" aria-hidden="true"
              onError={() => setImageErrors(prev => new Set(prev).add(game.path))} />
          ) : (
            <div className="selected-cover-copy selected-cover-copy--fallback" aria-hidden="true">
              <img src="/peligames.svg" alt="" />
              {viewMode === "grid" && <span>{game.name}</span>}
            </div>
          )}
          {runningBadge(game)}
          {managed && <GamePlatformBadge platform={game.nexus_platform} />}
          {viewMode === "list" && <><span className="selected-game-placeholder-name">{game.name}</span><span className="selected-game-list-note">{t("gameGrid", "selectedGame")}</span></>}
        </div>
      );
    }

    return (
      <div
        className={`library-game-card ${viewMode === "list" ? "library-game-card--list" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "shatter" ? "is-holding-launch" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "particles" ? "is-holding-particles" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "shrink" ? "is-holding-shrink" : ""}`}
        key={`${gameViewKey(game)}-${viewMode}-${performanceMode ? "performance" : "elegant"}`} 
        onPointerDown={event => startHold(event, game)}
        onPointerMove={event => {
          const active = hold.current;
          if (active && active.pointer === event.pointerId && Math.hypot(event.clientX - active.x, event.clientY - active.y) > 10) {
            suppressHoldClick.current = active.key;
            cancelHold();
          }
        }}
        onPointerLeave={cancelHold}
        onDragStart={event => { event.preventDefault(); }}
        onClick={event => {
          if (suppressHoldClick.current === gameViewKey(game)) { suppressHoldClick.current = null; event.preventDefault(); return; }
          onSelectGame(game, event.currentTarget.querySelector<HTMLElement>(".library-cover-thumbnail") ?? undefined);
        }}
        onMouseEnter={handleMouseEnter}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        style={{ 
          background: "rgba(0,0,0,0.3)", 
          borderRadius: "8px", 
          overflow: "hidden", 
          cursor: "pointer",
          transition: "box-shadow 0.1s ease-out",
          border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)",
          position: "relative",
          transformStyle: "flat"
        }}
      >
      {holdingKey === gameViewKey(game) && launchEffect === "smooth" && <div className="library-hold-progress" aria-hidden="true"><span /></div>}
          {holdingKey === gameViewKey(game) && launchEffect === "aquarium" && <CoverWaterFill reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "particles" && <CoverHoldParticles reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "penguin" && <CoverWalkingPenguin reducedMotion={performanceMode} />}
      {holdingKey === gameViewKey(game) && launchEffect === "shatter" && !isRunning(game) && <div className="library-hold-launch" aria-hidden="true"><svg className="library-hold-cracks" viewBox="0 0 100 100" preserveAspectRatio="none">{coverCrackPaths.map((path, index) => <g key={path}><path className="cover-crack-shadow" d={path} pathLength={1} style={{ animationDelay: `${index / coverCrackPaths.length * .52}s` }} /><path d={path} pathLength={1} style={{ animationDelay: `${index / coverCrackPaths.length * .52}s` }} /></g>)}</svg></div>}

      {runningBadge(game)}
      <div className="glare" style={{
        position: "absolute", top: 0, left: 0, right: 0, bottom: 0,
        pointerEvents: "none", opacity: 0, transition: "opacity 0.2s ease-out", zIndex: 10
      }} />
      {/* The same portrait owns the shared layout in both views; never the full list row. */}
      <motion.div className="library-cover-thumbnail" layout={performanceMode ? false : "preserve-aspect"} layoutDependency={viewMode}
        onContextMenu={(e) => { e.preventDefault(); setContextMenu({ x: e.clientX, y: e.clientY, game }); }}
        layoutId={performanceMode ? undefined : `cover-${gameViewKey(game)}`}>
      {!imageErrors.has(game.path) && game.cover_url ? (
        <img 
          src={game.cover_url} 
          alt={game.name}
          decoding="async"
          onLoad={event => { if (!performanceMode && game.cover_url) warmAmbientCover(game.cover_url, event.currentTarget); }}
          style={{ width: "100%", aspectRatio: "2/3", objectFit: "cover", display: "block" }} 
          onError={async () => {
            setImageErrors(prev => new Set(prev).add(game.path));
            if (!hasCustomCover(game) && !fallbackAttempted.has(game.path)) {
              setFallbackAttempted(prev => new Set(prev).add(game.path));
              try {
                if (managedLibrary?.onCoverFallback) { await managedLibrary.onCoverFallback(game); return; }
                const newCover = await invoke<string | null>("fetch_steamgriddb_cover_command", { name: game.name, apiKey: STEAMGRIDDB_API_KEY });
                if (newCover && !hasCustomCover(game)) {
                  setGames(prev => prev.map(g => g.path === game.path ? { ...g, cover_url: newCover, automatic_cover_url: newCover } : g));
                  setImageErrors(prev => {
                    const next = new Set(prev);
                    next.delete(game.path);
                    return next;
                  });
                }
              } catch (e) {
                console.error("Failed to fetch fallback cover", e);
              }
            }
          }}
        />
      ) : null}
      <div 
        className="fallback-cover" 
        style={{ 
          width: "100%", aspectRatio: "2/3", 
          background: "linear-gradient(135deg, #1e3a8a, #312e81)", 
          display: (!game.cover_url || imageErrors.has(game.path)) ? "flex" : "none", 
          position: "relative",
          textAlign: "center"
        }}
      >
        <span style={{ position: "absolute", top: "1rem", left: "0.5rem", right: "0.5rem", fontWeight: "bold", fontSize: "0.9rem", textShadow: "0 2px 4px rgba(0,0,0,0.8)" }}>{game.name}</span>
        <img src="/peligames.svg" alt="Icon" style={{ position: "absolute", top: "50%", left: "50%", transform: "translate(-50%, -50%)", width: "48px", height: "48px", opacity: 0.5 }} />
      </div>
      
      {managed && <GamePlatformBadge platform={game.nexus_platform} />}
      </motion.div>
      <div className="game-list-details"><span>{game.name}</span></div>
      <div className="game-launcher-badge" style={{ position: "absolute", bottom: 0, left: 0, right: 0, padding: "0.5rem", background: "linear-gradient(to top, rgba(0,0,0,0.9), transparent)" }}>
        {game.launcher === "PeliGames" && <span className="peligames-executable-label" title={game.executable}>{game.executable?.split(/[\\/]/).pop()}</span>}
        <div style={{ fontSize: "0.7rem", color: "var(--tone-9ca3af, #9ca3af)", display: "flex", justifyContent: "space-between" }}>
          <span style={{ display: "flex", alignItems: "center", gap: "0.3rem" }}>
            <LauncherIcon launcher={game.launcher} size={12} />
            {game.launcher}
          </span>
        </div>
      </div>
    </div>
  );
  };

  // Project the whole cell (including its background) when the details panel resizes.
  // The inner div remains responsible for mouse tilt; it never competes with layout transforms.
  const renderCard = (game: GameInfo) => (
    <motion.div key={gameViewKey(game)} className={`library-card-cell ${brokenCards.has(gameViewKey(game)) ? "library-card-cell--shattered" : ""}`} aria-hidden={brokenCards.has(gameViewKey(game)) || undefined} data-cover-key={gameViewKey(game)} layout={performanceMode ? false : "position"} layoutDependency={layoutRevision}
      transition={{ layout: { duration: 0.3, ease: [0.4, 0, 0.2, 1] } }}>
      {renderCardContent(game)}
    </motion.div>
  );

  const cacheScope = managedLibrary ? "nexus" : libraryScope;
  // null means no successful scan yet; an empty saved library is still valid.
  const hasLibraryCache = librarySnapshots.current[cacheScope] !== null;
  const refreshingLibrary = managedLibrary
    ? !managedLibrary.loaded || Boolean(managedLibrary.scanning)
    : loading || !libraryLoaded || !ownLoaded;
  const liveGames = managedLibrary ? managedLibrary.games
    : libraryScope === "own" ? games.filter(game => game.launcher === "PeliGames") : games;
  // A pending scan must not replace the last complete library with an empty or
  // partially discovered result. New entries can still appear during the scan.
  const previousGames = librarySnapshots.current[cacheScope] ?? [];
  const displayedGames = refreshingLibrary
    ? [...new Map([...previousGames, ...liveGames].map(game => [gameViewKey({ ...game, library_view: cacheScope }), game])).values()]
    : liveGames;
  useEffect(() => {
    if (refreshingLibrary || libraryError || managedLibrary?.error) return;
    librarySnapshots.current[cacheScope] = liveGames;
    saveGameLibrary(liveGames, cacheScope);
  }, [cacheScope, refreshingLibrary, games, managedLibrary?.games, managedLibrary?.error, libraryError]);
  const scopedGames = applyCustomCovers(displayedGames
    .map(game => ({ ...game, library_view: cacheScope })))
    .filter(game => !removedGames.has(gameViewKey(game)));
  const filteredGames = scopedGames.filter(g => g.name.toLowerCase().includes(searchQuery.toLowerCase()));
  // Re-measure for actual library arrangement changes, not focus/polling renders.
  const layoutRevision = JSON.stringify([viewMode, filteredGames.map(gameViewKey), [...favorites], favoritesCollapsed, collapsedCats]);
  const favoriteGames = filteredGames.filter(game => favorites.has(gameViewKey(game)));
  const remainingGames = filteredGames.filter(game => !favorites.has(gameViewKey(game)));
  const steamGames = remainingGames.filter(g => g.launcher === "Steam");
  const otherGames = remainingGames.filter(g => g.launcher !== "Steam");
  // Mod operations share the busy flag, but must not unmount the loaded covers.
  const scanning = managedLibrary
    ? !managedLibrary.loaded || (!hasLibraryCache && Boolean(managedLibrary.scanning))
    : loading || (!hasLibraryCache && !ownLoaded) || (libraryScope === "own" && !ownLoaded);
  // A first scan stays visible until complete. Only a previously cached library
  // can replace the pelican after three seconds while scanning continues.
  const [scanGraceOver, setScanGraceOver] = useState(false);
  useEffect(() => {
    if (!scanning || !hasLibraryCache) { setScanGraceOver(false); return; }
    const timer = window.setTimeout(() => setScanGraceOver(true), 3000);
    return () => window.clearTimeout(timer);
  }, [scanning, hasLibraryCache, cacheScope]);
  const shownLibraries = useRef(new Set<string>());
  const showScanScreen = scanning && (!hasLibraryCache || (!scanGraceOver && !shownLibraries.current.has(cacheScope)));
  useEffect(() => {
    if (!showScanScreen) shownLibraries.current.add(cacheScope);
  }, [showScanScreen, cacheScope]);
  const scanSpinning = scanning || Boolean(managedLibrary?.scanning);
  const showEmptyOwnLibrary = !scanning && ownLoaded && (libraryScope === "own" || mode === "mods") && !searchQuery && filteredGames.length === 0;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <AnimatePresence>{teleport && <CoverLaunchTeleport value={teleport} reducedMotion={performanceMode} cancellingDisabled={launchCommitted} onCancel={() => cancelPendingLaunch.current?.()} />}</AnimatePresence>
      <div className={`grid-header`} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1rem", flexWrap: "wrap", gap: "1rem", paddingRight: "0.5rem" }}>
        
        {/* Right Controls */}
        {(libraryError || managedLibrary?.error) && <TimedFeedback>{libraryError || managedLibrary?.error}</TimedFeedback>}
        <div className="library-toolbar-controls">
          
          {/* Search Bar */}
          <div className={`library-search ${searchExpanded ? "expanded" : ""}`} onBlur={event => {
            if (!event.currentTarget.contains(event.relatedTarget as Node)) setSearchExpanded(false);
          }} onKeyDown={event => {
            if (event.key === "Escape") { setSearchExpanded(false); searchButton.current?.focus(); }
          }}>
            <button ref={searchButton} type="button" className={`library-search-button ${searchQuery ? "has-query" : ""}`}
              aria-label={t("gameGrid", "search")} aria-expanded={searchExpanded} aria-controls={searchId}
              onClick={() => setSearchExpanded(value => !value)}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
            </button>
            <div id={searchId} className="library-search-field">
            <svg style={{ position: "absolute", left: "0.5rem", top: "50%", transform: "translateY(-50%)", color: "var(--tone-94a3b8, #94a3b8)" }} width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
            <input ref={searchInput} aria-label={t("gameGrid", "search")}
              type="text" 
              placeholder={t("gameGrid", "search")} 
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              style={{ width: "100%", padding: "0.4rem 2rem 0.4rem 2rem", background: "rgba(0,0,0,0.3)", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)", borderRadius: "6px", color: "var(--tone-f8fafc, #f8fafc)", outline: "none", fontSize: "0.85rem", boxSizing: "border-box" }}
            />
            {searchQuery && (
              <button
                aria-label={t("gameGrid", "clearSearch")} onClick={() => { setSearchQuery(""); searchInput.current?.focus(); }}
                style={{ position: "absolute", right: "0.5rem", top: "50%", transform: "translateY(-50%)", background: "transparent", border: "none", color: "var(--tone-94a3b8, #94a3b8)", cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "center", padding: 0 }}
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
              </button>
            )}
          </div>
            </div>
          {managedLibrary ? <div className="library-mode-buttons"><HoverTooltip text={managedLibrary.accountLabel}><button type="button" className="library-mode-button nexus-library-title" aria-label={managedLibrary.accountLabel} onClick={managedLibrary.onAccount}><img className="nexus-icon" src="/nexus-mods.svg" alt="" /><span className="nexus-library-wordmark" aria-hidden="true"><span>NEXUS</span><strong>MODS</strong></span></button></HoverTooltip>{managedLibrary.loaded && (!managedLibrary.busy || managedLibrary.games.length > 0) && <GameEntryButton addLabel={managedLibrary.addLabel} disabled={modeDisabled || managedLibrary.busy} onClick={managedLibrary.onAdd} />}{managedLibrary.downloadsAction}</div> : <GameModeSelector modLibrary={libraryScope === "all"} onHome={onHome} mode={mode} open={detailsOpen} onChange={onModeChange} disabled={modeDisabled} />}
          <div className="library-folder-actions" style={{ display: "flex", gap: "0.5rem" }}>
            {!managedLibrary && libraryScope === "all" && <HoverTooltip text={t("gameGrid", "addFolder")}><button
              className="library-mode-button"
              onClick={handleAddCustomFolder}
              style={{ background: "transparent", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)", color: "var(--tone-94a3b8, #94a3b8)", cursor: "pointer", padding: "0.4rem 0.6rem", borderRadius: "6px", display: "flex", alignItems: "center", gap: "0.4rem", transition: "all 0.2s" }}
              onMouseOver={(e) => { e.currentTarget.style.background = "rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)"; e.currentTarget.style.color = "var(--tone-f8fafc, #f8fafc)"; }}
              onMouseOut={(e) => { e.currentTarget.style.background = "transparent"; e.currentTarget.style.color = "var(--tone-94a3b8, #94a3b8)"; }}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M2 12a10 10 0 1 0 20 0 10 10 0 1 0-20 0Z"/><path d="M8 12h8"/><path d="M12 8v8"/></svg>
              <span style={{ fontSize: "0.85rem", fontWeight: 500 }}>{t("gameGrid", "folder")}</span>
            </button></HoverTooltip>}
            <HoverTooltip text={t("gameGrid", "scanLibrary")}><button 
              className="library-mode-button"
              onClick={() => void fetchGames()}
              disabled={scanSpinning || managedLibrary?.busy}
              style={{ background: "transparent", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)", color: loading ? "white" : "var(--tone-94a3b8, #94a3b8)", cursor: loading ? "not-allowed" : "pointer", padding: "0.4rem 0.6rem", borderRadius: "6px", display: "flex", alignItems: "center", gap: "0.4rem", transition: "all 0.2s", opacity: loading ? 0.7 : 1 }}
              onMouseOver={(e) => { if (!loading) { e.currentTarget.style.background = "rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)"; e.currentTarget.style.color = "var(--tone-f8fafc, #f8fafc)"; } }}
              onMouseOut={(e) => { if (!loading) { e.currentTarget.style.background = "transparent"; e.currentTarget.style.color = "var(--tone-94a3b8, #94a3b8)"; } }}
            >
              <motion.div animate={{ rotate: scanSpinning && !performanceMode ? 360 : 0 }} transition={{ repeat: scanSpinning && !performanceMode ? Infinity : 0, duration: 1, ease: "linear" }} style={{ display: "flex" }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/></svg>
              </motion.div>
              <span style={{ fontSize: "0.85rem", fontWeight: 500 }}>{t("gameGrid", "scan")}</span>
            </button></HoverTooltip>
          </div>

          {(managedLibrary || libraryScope === "all") && <HoverTooltip text={t("gameModes", "goHome")}><button type="button" className="library-mode-button library-home-button" disabled={modeDisabled || managedLibrary?.busy} onClick={onHome}><img src="/peligames.svg" alt="" /><span>{t("gameModes", "home")}</span></button></HoverTooltip>}
          <div style={{ width: "1px", height: "20px", background: "rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)" }}></div>

          <div className="library-view-toggle" role="group" aria-label={t("gameGrid", "viewMode")}>
            <HoverTooltip text={t("gameGrid", "gridView")}><button type="button" className={viewMode === "grid" ? "active" : ""}
              onClick={() => setViewMode("grid")} aria-pressed={viewMode === "grid"}
              aria-label={t("gameGrid", "gridView")}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>
            </button></HoverTooltip>
            <HoverTooltip text={t("gameGrid", "listView")}><button type="button" className={viewMode === "list" ? "active" : ""}
              onClick={() => setViewMode("list")} aria-pressed={viewMode === "list"}
              aria-label={t("gameGrid", "listView")}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/></svg>
            </button></HoverTooltip>
          </div>

        </div>
      </div>

      <motion.div layoutScroll className={`game-grid-container game-library--${viewMode}`} style={{ flex: 1, minHeight: 0, overflowY: "auto", marginTop: "1rem" }}>
        {showScanScreen ? <div className="empty-library-state"><NexusLibraryLoading reducedMotion={performanceMode} /></div> : <>
        {filteredGames.length === 0 && !scanSpinning && (managedLibrary ? <div className="empty-library-state">
          {searchQuery ? <p role="status">{t("gameGrid", "noSearchResults")}</p>
            : managedLibrary.error ? <button className="btn btn-secondary" onClick={managedLibrary.onRefresh}>{t("gameGrid", "scan")}</button>
            : <div className="empty-library-action"><button type="button" className="empty-library-add" onClick={managedLibrary.onAdd} aria-label={managedLibrary.addLabel}><MenuIcon name="plus" /></button><div className="empty-library-message" role="status"><h3>{managedLibrary.empty}</h3><p>{managedLibrary.hint}</p></div></div>}
        </div> : showEmptyOwnLibrary ?
          <div className="empty-library-state">
            <div className="empty-library-actions">
              <div className="empty-library-action">
                <button type="button" className="empty-library-add" disabled={modeDisabled}
                  aria-label={t("gameModes", "entryTitle")} aria-describedby="empty-library-hint"
                  onClick={() => onModeChange("install")}><MenuIcon name="plus" /></button>
                <div id="empty-library-hint" className="empty-library-message" role="status">
                  <h3>{t("gameModes", "entryTitle")}</h3>
                  <p>{t("gameGrid", "noOwnGamesMessage")}</p>
                  <p className="empty-library-hint">{t("gameGrid", "noOwnGamesHint")}</p>
                </div>
              </div>
              <div className="empty-library-action">
                <button type="button" className="empty-library-add" disabled={modeDisabled}
                  aria-label={t("gameModes", "mods")} aria-describedby="empty-library-mod-hint"
                  onClick={() => onModeChange("mods")}><MenuIcon name="puzzle" /></button>
                <div id="empty-library-mod-hint" className="empty-library-message">
                  <h3>{t("gameModes", "mods")}</h3>
                  <p>{t("gameGrid", "emptyModDescription")}</p>
                  <p className="empty-library-hint">{t("gameGrid", "noOwnGamesHint")}</p>
                </div>
              </div>
            </div>
          </div> : <div role="status" style={{ textAlign: "center", padding: "2rem", color: "var(--tone-94a3b8, #94a3b8)" }}>
            <p>{searchQuery ? t("gameGrid", "noSearchResults") : libraryScope === "own" ? t("gameGrid", "noOwnGames") : t("app", "noGamesFound")}</p>
          </div>)}
        {favoriteGames.length > 0 && <section className="favorites-category" style={{ marginBottom: "1.5rem" }} aria-label={t("gameGrid", "favorites")}>
          <div className="launcher-category-header favorites-category-header" style={{ marginBottom: favoritesCollapsed ? 0 : "1rem" }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ transform: favoritesCollapsed ? "rotate(-90deg)" : "rotate(0deg)", transition: "transform .2s", marginRight: ".5rem" }} aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
            <span className="category-launcher-icon"><MenuIcon name="heart" /></span>
            <h3><button type="button" className="launcher-category-toggle" aria-expanded={!favoritesCollapsed} onClick={() => setFavoritesCollapsed(value => !value)}>{t("gameGrid", "favorites")}</button> <span className="favorites-category-count">{favoriteGames.length}</span></h3>
          </div>
          {!favoritesCollapsed && <div className={`game-collection game-collection--${viewMode}`}>{favoriteGames.map(game => renderCard(game))}</div>}
        </section>}
        {steamGames.length > 0 && (
          <div style={{ marginBottom: "1.5rem" }}>
            <div 
              className="launcher-category-header"
              style={{ 
                display: "flex", alignItems: "center", 
                marginBottom: collapsedCats["Steam"] ? "0" : "1rem", userSelect: "none" 
              }}
            >
              <svg 
                width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#60a5fa" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"
                style={{ transform: collapsedCats["Steam"] ? "rotate(-90deg)" : "rotate(0deg)", transition: "transform 0.2s", marginRight: "0.5rem" }}
              >
                <path d="m6 9 6 6 6-6"/>
              </svg>
              <span className="category-launcher-icon" style={{ color: "var(--tone-60a5fa, #60a5fa)" }}><LauncherIcon launcher="Steam" /></span>
              <h3 style={{ margin: 0, fontSize: "1.1rem", color: "var(--tone-60a5fa, #60a5fa)" }}>
                <button type="button" className="launcher-category-toggle" aria-expanded={!collapsedCats["Steam"]}
                  onClick={() => setCollapsedCats(prev => ({ ...prev, Steam: !prev.Steam }))}>Steam</button> <span style={{ fontSize: "0.85rem", color: "var(--tone-6b7280, #6b7280)", fontWeight: "normal" }}>{steamGames.length}</span>
              </h3>
            </div>
            
            {!collapsedCats["Steam"] && (
              <div className={`game-collection game-collection--${viewMode}`}>
                {steamGames.map(game => renderCard(game))}
              </div>
            )}
          </div>
        )}

        {(() => {
          const grouped: Record<string, typeof otherGames> = {};
          otherGames.forEach(game => {
            if (game.launcher === "Manual") {
              const folder = customFolders.find(f => game.path.startsWith(f));
              const catName = folder ? folder.split(/[\\/]/).pop() || folder : "Avulsos";
              if (!grouped[catName]) grouped[catName] = [];
              grouped[catName].push(game);
            } else {
              const catName = game.launcher;
              if (!grouped[catName]) grouped[catName] = [];
              grouped[catName].push(game);
            }
          });

          return Object.entries(grouped).map(([catName, gms]) => (
            <div key={catName} style={{ marginBottom: "1.5rem" }}>
              <div 
                className="launcher-category-header"
                style={{ 
                  display: "flex", alignItems: "center", 
                  marginBottom: collapsedCats[catName] ? "0" : "1rem", userSelect: "none" 
                }}
              >
                <svg 
                  width="16" height="16" viewBox="0 0 24 24" fill="none" stroke={catName === "PeliGames" ? "var(--accent-soft)" : "var(--tone-c084fc, #c084fc)"} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"
                  style={{ transform: collapsedCats[catName] ? "rotate(-90deg)" : "rotate(0deg)", transition: "transform 0.2s", marginRight: "0.5rem" }}
                >
                  <path d="m6 9 6 6 6-6"/>
                </svg>
                {gms.some(game => game.launcher === "Heroic") && <span className="category-launcher-icon" style={{ color: "var(--tone-c084fc, #c084fc)" }}><LauncherIcon launcher="Heroic" /></span>}
                {catName === "PeliGames" && <span className="category-launcher-icon"><LauncherIcon launcher="PeliGames" /></span>}
                <h3 style={{ margin: 0, fontSize: "1.1rem", color: catName === "PeliGames" ? "var(--accent-soft)" : "var(--tone-c084fc, #c084fc)" }}>
                  <button type="button" className="launcher-category-toggle" aria-expanded={!collapsedCats[catName]}
                    onClick={() => setCollapsedCats(prev => ({ ...prev, [catName]: !prev[catName] }))}>{catName}</button> <span style={{ fontSize: "0.85rem", color: "var(--tone-6b7280, #6b7280)", fontWeight: "normal" }}>{gms.length}</span>
                </h3>
              </div>
              
              {!collapsedCats[catName] && (
                <div className={`game-collection game-collection--${viewMode}`}>
                  {gms.map(game => renderCard(game))}
                </div>
              )}
            </div>
          ));
        })()}
        </>}
      </motion.div>
      {contextMenu && createPortal(<GameCoverMenu game={contextMenu.game} x={contextMenu.x} y={contextMenu.y}
        blocked={coverActionsBlocked} onClose={() => setContextMenu(null)}
        onAction={(action, game) => { if (action === "scan") void fetchGames(); else onCoverAction(action, game); }} />, document.body)}

    </div>
  );
};

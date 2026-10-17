import { GameBannerImage } from "./GameBannerImage";
import { CoverHoldProgress } from "./CoverHoldProgress";
import { useAccentTheme } from "../theme/ThemeProvider";
import { createPortal } from "react-dom";
import { CoverFreeze } from "./CoverFreeze";
import { CoverTuxRise } from "./CoverTuxRise";
import { CoverHoldParticles } from "./CoverHoldParticles";
import { CoverWaterFill } from "./CoverWaterFill";
import { CoverLaunchTeleport, coverCrackPaths, type CoverTeleport } from "./CoverLaunchTeleport";
import { ModernLibraryFilters } from "./modern/ModernLibraryFilters";
import { GamePlatformBadge } from "./GamePlatformBadge";
import { NexusLibraryLoading } from "../modules/nexus/NexusLibraryLoading";
import { useFavorites, toggleFavorite } from "../services/favorites";
import type { CoverGameAction } from "./CoverGameActions";
import { GameCoverMenu } from "./GameCoverMenu";
import { useRemovedLibraryGames, restoreRemovedLibraryFolder } from "../services/libraryVisibility";
import { gameViewKey } from "../services/gameIdentity";
import { TimedFeedback } from "./TimedFeedback";
import { MenuIcon } from "./MenuIcon";
import { STEAMGRIDDB_API_KEY } from "../services/coverConfig";
import { hasCustomCover } from "../services/customCovers";
import { forgetAutomaticCover, rememberAutomaticCover } from "../services/automaticCovers";
import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n/I18nContext";
import { AnimatePresence, motion } from "framer-motion";
import { usePerformanceMode } from "./EffectsContext";
import { applyCustomCovers, readGameLibrary, saveGameLibrary } from "../services/gameLibraryCache";
import { warmAmbientCover } from "../services/ambientCoverCache";
import type { GamePanelMode } from "../types/gamePanel";
import { LauncherIcon } from "./LauncherIcon";
import { openDirectoryPicker } from "../services/tauriService";
import { launcherCoverFallback } from "../services/launcherInstallation";
import { listPeliGamesEntries } from "../services/installedLibrary";
import { activityOf, daysSince, formatPlaytime, loadGameActivity, type ActivityMap } from "../services/gameActivity";
import { SmoothImage } from "./SmoothImage";

const COVER_HOLD_FEEDBACK_DELAY = 250;
const COVER_HOLD_ANIMATION_DURATION = 1200;
const libraryNameOrder = new Intl.Collator("pt-BR", { sensitivity: "base", numeric: true });

export interface GameInfo {
  nexus_platform?: "native" | "proton";
  nexus_running?: boolean;
  nexus_starting?: boolean;
  /** ID of the Nexus library entry (several entries may share a folder). */
  nexus_id?: string;
  library_category?: "launchers";
  launcher_id?: string;
  external_id?: string;
  external_store?: string;
  flatpak?: boolean;
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
  onSelectGame: (game: GameInfo, portrait?: HTMLElement) => void;
  selectedGameKey?: string;
  runningGamePath?: string;
  startingGamePath?: string;
  mode: GamePanelMode;
  libraryScope: "own" | "all";
  modeDisabled?: boolean;
  onModeChange: (mode: GamePanelMode) => void;
  /** New interface: the side menu switches modes, and covers tilt less. */
  /** New interface: number of games, shown on the side menu. */
  onCountChange?: (count: number) => void;
}

export const GameGrid: React.FC<Props> = ({ onCoverAction, coverActionsBlocked, coverHoldBlocked, homeRevision, onSelectGame, selectedGameKey, runningGamePath, startingGamePath, mode, libraryScope, onModeChange, modeDisabled, managedLibrary, onCountChange }) => {
  const tilt = 6;
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
  const performanceMode = usePerformanceMode();
  const [cachedGames] = useState(() => readGameLibrary());
  const [initialSnapshots] = useState(() => ({
    all: cachedGames,
    own: readGameLibrary("own"),
    nexus: readGameLibrary("nexus"),
  }));
  const librarySnapshots = useRef(initialSnapshots);
  const [games, setGames] = useState<GameInfo[]>(cachedGames ?? []);
  const [libraryLoaded, setLibraryLoaded] = useState(cachedGames !== null);
  const startupHandled = useRef<string | null>(null);
  const scanInProgress = useRef(false);
  const activeScope = useRef(libraryScope);
  activeScope.current = libraryScope;
  const [scanPending, setScanPending] = useState(false);
  const pendingScanFolders = useRef<{ folders: string[]; background: boolean; scope: "own" | "all" } | null>(null);
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
    const refresh = () => { void refreshOwnLibrary(true); };
    window.addEventListener("peligamesLibraryChanged", refresh);
    return () => { ++ownRequest.current; window.removeEventListener("peligamesLibraryChanged", refresh); };
  }, [Boolean(managedLibrary)]);
  const [searchQuery, setSearchQuery] = useState("");
  const searchInput = useRef<HTMLInputElement>(null);
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
  // New interface: Ctrl+K focuses the library search.
  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => { if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") { event.preventDefault(); searchInput.current?.focus(); } };
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);
  // New interface: one grid filtered by store chips instead of collapsible categories.
  const [storeFilter, setStoreFilter] = useState("all");
  useEffect(() => { setStoreFilter("all"); }, [libraryScope, Boolean(managedLibrary)]);
  // New interface: last play and playtime from each launcher's own records,
  // refreshed when a session ends and when the window regains focus.
  const [activity, setActivity] = useState<ActivityMap>({});
  useEffect(() => {
    let alive = true;
    const refresh = () => { void loadGameActivity().then(value => { if (alive) setActivity(value); }); };
    refresh();
    window.addEventListener("focus", refresh);
    return () => { alive = false; window.removeEventListener("focus", refresh); };
  }, [runningGamePath, homeRevision]);
  const [contextMenu, setContextMenu] = useState<{ x: number, y: number, game: GameInfo } | null>(null);
  const { t } = useI18n();
  useEffect(() => { setSearchQuery(""); setContextMenu(null); }, [libraryScope]);

  useEffect(() => {
    const handleClick = () => setContextMenu(null);
    document.addEventListener("click", handleClick);
    return () => document.removeEventListener("click", handleClick);
  }, []);

  const [fallbackAttempted, setFallbackAttempted] = useState<Set<string>>(new Set());

  const fetchGames = async (folders: string[] = customFolders, background = false, scope = libraryScope) => {
    if (managedLibrary) { managedLibrary.onRefresh(); return; }
    if (scanInProgress.current) {
      // A focus refresh must not downgrade a pending explicit scan.
      if (!pendingScanFolders.current || pendingScanFolders.current.scope !== scope || !background) pendingScanFolders.current = { folders, background, scope };
      return;
    }
    scanInProgress.current = true;
    setScanPending(true);
    // Restoring the window refreshes data without unmounting the visible cards.
    if (!background) {
      setLoading(true);
      setImageErrors(new Set());
      setFallbackAttempted(new Set());
    }
    try {
      const [scanned, external] = scope === "own"
        ? [await invoke<GameInfo[]>("scan_external_library"), [] as GameInfo[]]
        : await Promise.all([invoke<GameInfo[]>("scan_installed_games", { apiKey: STEAMGRIDDB_API_KEY, customFolders: folders }), invoke<GameInfo[]>("scan_external_library").catch(() => [] as GameInfo[])]);
      const result = scanned.map(game => {
        // The DLSS scan points at the executable's folder, often below the game's
        // root (bin/x64_dx12…): match Steam games by their ID, others by containment.
        const inside = (root: string) => { const base = root.replace(/\/+$/, ""); return game.path === base || game.path.startsWith(base + "/"); };
        const settings = external.find(entry => entry.launcher === game.launcher && (entry.path === game.path || (game.launcher === "Steam" && Boolean(game.app_id) && entry.app_id === game.app_id) || (game.launcher !== "Steam" && inside(entry.path))));
        // Never the launcher's folder: the DLSS library finds the game folder its own
        // way (the executable that loads the graphics API) and keeps manual choices.
        return settings ? {...game, external_id: settings.external_id, advanced: settings.advanced, proton: settings.proton, prefix: settings.prefix} : game;
      });
      if (scope !== activeScope.current) return;
      setGames(applyCustomCovers([...result.filter(game => game.launcher !== "PeliGames"), ...ownEntries.current]));
      await refreshOwnLibrary(true);
      setLibraryLoaded(true);
    } catch (e) {
      console.error("Failed to scan games", e);
      setLibraryError(String(e));
      setLibraryLoaded(true);
    } finally {
      scanInProgress.current = false;
      const pending = pendingScanFolders.current;
      pendingScanFolders.current = null;
      if (pending) void fetchGames(pending.folders, pending.background, pending.scope);
      else { setLoading(false); setScanPending(false); }
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
    if (startupHandled.current !== libraryScope) {
      startupHandled.current = libraryScope;
      void fetchGames(customFolders, cachedGames !== null);
      if (cachedGames !== null) void invoke("log_cached_library", { count: cachedGames.length }).catch(() => {});
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
    if (viewMode === "list" || performanceMode || event.currentTarget.classList.contains("is-away")) return;
    hoverBounds.current = { card: event.currentTarget, rect: event.currentTarget.getBoundingClientRect() };
  };
  const handleMouseMove = (event: React.MouseEvent<HTMLDivElement>) => {
    if (viewMode === "list" || performanceMode || event.currentTarget.classList.contains("is-away")) return;
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
      const rotateX = ((y - rect.height / 2) / (rect.height / 2)) * -tilt;
      const rotateY = ((x - rect.width / 2) / (rect.width / 2)) * tilt;
      // A short transition smooths the pointer's steps without lagging behind it.
      card.style.transition = "transform .12s ease-out, box-shadow .25s ease, border-color .25s ease";
      card.style.transform = `perspective(900px) rotateX(${rotateX}deg) rotateY(${rotateY}deg) scale3d(1.04, 1.04, 1.04)`;
      card.style.zIndex = "20";
      card.style.boxShadow = "0 22px 44px rgba(0, 0, 0, .55), 0 0 26px rgba(var(--accent-rgb), .28)";
      card.style.borderColor = "rgba(var(--accent-rgb), 0.7)";
      // Glass reflection (see .glare in modern.css): a soft highlight under the
      // pointer and a diagonal sheen that crosses the cover the opposite way.
      const glare = card.querySelector('.glare') as HTMLDivElement;
      if (glare) {
        const px = x / rect.width * 100, py = y / rect.height * 100;
        glare.style.setProperty("--glare-x", `${px}%`);
        glare.style.setProperty("--glare-y", `${py}%`);
        glare.style.setProperty("--glare-sheen", `${120 - (px * .7 + py * .3) * 1.4}%`);
        glare.style.opacity = "1";
      }
    });
  };

  const handleMouseLeave = (e: React.MouseEvent<HTMLDivElement>) => {
    const card = e.currentTarget;
    if (hoverFrame.current !== null) cancelAnimationFrame(hoverFrame.current);
    hoverFrame.current = null;
    hoverBounds.current = null;
    // Settle back slowly, like glass tilting back into place.
    card.style.transition = "transform .45s cubic-bezier(.2, .8, .2, 1), box-shadow .35s ease, border-color .35s ease";
    card.style.transform = (viewMode === "list" || performanceMode) ? "none" : `perspective(900px) rotateX(0deg) rotateY(0deg) scale3d(1, 1, 1)`;
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
  const coverPress = useRef<{ key: string; startedAt: number } | null>(null);
  const launchBlocked = useRef(coverHoldBlocked ?? coverActionsBlocked);
  launchBlocked.current = coverHoldBlocked ?? coverActionsBlocked;
  const runningSnapshot = useRef({ path: runningGamePath, games: managedLibrary?.games });
  runningSnapshot.current = { path: runningGamePath, games: managedLibrary?.games };
  const isRunning = (game: GameInfo) => game.path === runningSnapshot.current.path || Boolean(runningSnapshot.current.games?.find(entry => entry.path === game.path && entry.launcher === game.launcher)?.nexus_running);
  const launchFromHold = useRef(onCoverAction);
  launchFromHold.current = onCoverAction;
  const cancelHold = () => {
    const press = coverPress.current;
    coverPress.current = null;
    // A cancelled hold must not become the browser's trailing click. Keep
    // ordinary quick clicks available even when cover launching is disabled.
    if (press && performance.now() - press.startedAt >= COVER_HOLD_FEEDBACK_DELAY) suppressHoldClick.current = press.key;
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
    if (event.button !== 0 || !event.isPrimary) return;
    coverPress.current = { key: gameViewKey(game), startedAt: performance.now() };
    if (launchEffect === "disabled" || launchBlocked.current
      || !(managed || game.external_id || game.launcher === "Steam" || game.launcher === "PeliGames")) return;
    const key = gameViewKey(game);
    const stopping = isRunning(game);
    if (!stopping && coverActionsBlocked) return;
    const source = (event.currentTarget.closest(".library-card-cell") ?? event.currentTarget).getBoundingClientRect();
    // A lightweight DOM copy keeps the frame, badges and image together; no
    // screenshot encoding or game data processing runs during the gesture.
    const preview = !stopping ? event.currentTarget.cloneNode(true) as HTMLElement : null;
    preview?.classList.remove("is-holding-launch", "is-shattering-launch", "is-holding-particles", "is-holding-shrink", "is-holding-freeze");
    preview?.querySelectorAll(".library-cover-shatter, .library-hold-launch, .cover-freeze").forEach(node => node.remove());
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
      const launchDelay = (launchEffect === "smooth" || launchEffect === "penguin" || launchEffect === "shrink") ? 3500 : 4050;
      setTeleport({ game, source, preview, delay: launchDelay, effect: launchEffect === "freeze" ? "freeze" : launchEffect === "shrink" ? "shrink" : launchEffect === "penguin" ? "penguin" : launchEffect === "particles" ? "particles" : launchEffect === "aquarium" ? "aquarium" : launchEffect === "smooth" ? "smooth" : "shatter" });
      // Give the enlarged cover a cancellation window before launching, including
      // Steam games whose external startup cannot be reliably cancelled afterwards.
      const ready = new Promise<boolean>(resolve => {
        const timer = window.setTimeout(() => {
          cancelPendingLaunch.current = null;
          resolve(true);
        }, launchDelay);
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
    }, COVER_HOLD_FEEDBACK_DELAY + COVER_HOLD_ANIMATION_DURATION);
    hold.current = { key, pointer: event.pointerId, x: event.clientX, y: event.clientY, timer, target: event.currentTarget };
    // Every effect waits for an intentional hold, rather than animating on a click.
    // Keep the full animation duration after this grace period.
    hold.current.feedbackTimer = window.setTimeout(() => {
      const active = hold.current;
      if (!active || active.key !== key || launchBlocked.current) return;
      if (launchEffect === "shrink") {
        // Shrinking must not make a stationary pointer leave the shrinking hit area.
        active.target.setPointerCapture(active.pointer);
      }
      setHoldingKey(key);
    }, COVER_HOLD_FEEDBACK_DELAY);
  };

  /** "Jogado hoje · 46 h" from the launcher's records; empty when unknown. */
  const playedLabel = (game: GameInfo) => {
    const record = activityOf(activity, game);
    if (!record) return "";
    const days = record.last_played > 0 ? daysSince(record.last_played) : -1;
    const when = days < 0 ? "" : days === 0 ? t("modern", "playedToday") : days === 1 ? t("modern", "playedYesterday") : t("modern", "playedDaysAgo").replace("{n}", String(days));
    return [when, formatPlaytime(record.minutes)].filter(Boolean).join(" · ");
  };
  const isPreparing = (game: GameInfo) => (game.nexus_starting || game.path === startingGamePath) && !(game.nexus_running || game.path === runningGamePath);
  const runningBadge = (game: GameInfo) => (game.nexus_running || game.path === runningGamePath) && <span className="library-running-badge" role="status"><i aria-hidden="true" />{t("gameExecution", "running")}</span>;
  const renderCardContent = (game: GameInfo) => {
    if (brokenCards.has(gameViewKey(game))) return null;
    // List view: a separate "selected" row. Grid view keeps the same card and image
    // (only a class turns it grey), so returning from the details swaps nothing.
    if (gameViewKey(game) === selectedGameKey && viewMode === "list") {
      return (
        <div 
          key={gameViewKey(game)}
          className={`game-card-placeholder ${isPreparing(game) ? "is-preparing-game" : ""} ${viewMode === "list" ? "game-card-placeholder--list" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "shatter" ? "is-holding-launch" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "particles" ? "is-holding-particles" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "shrink" ? "is-holding-shrink" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "freeze" ? "is-holding-freeze" : ""}`}
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
          {holdingKey === gameViewKey(game) && launchEffect === "smooth" && <CoverHoldProgress />}
          {holdingKey === gameViewKey(game) && launchEffect === "freeze" && <CoverFreeze reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "aquarium" && <CoverWaterFill reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "particles" && <CoverHoldParticles reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "penguin" && <CoverTuxRise reducedMotion={performanceMode} />}
          {game.cover_url && !imageErrors.has(game.path) ? (
            <SmoothImage className="selected-cover-copy" src={game.cover_url} alt="" aria-hidden="true"
              onError={() => setImageErrors(prev => new Set(prev).add(game.path))} />
          ) : (
            <div className="selected-cover-copy selected-cover-copy--fallback" aria-hidden="true">
              <img src="/peligames.svg" alt="" />
            </div>
          )}
          {runningBadge(game)}
          <GamePlatformBadge game={game} />
          {viewMode === "list" && <><span className="selected-game-placeholder-name">{game.name}</span><span className="selected-game-list-note">{t("gameGrid", "selectedGame")}</span></>}
        </div>
      );
    }

    return (
      <div
        className={`library-game-card${gameViewKey(game) === selectedGameKey ? " is-away" : ""} ${isPreparing(game) ? "is-preparing-game" : ""} ${viewMode === "list" ? "library-game-card--list" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "shatter" ? "is-holding-launch" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "particles" ? "is-holding-particles" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "shrink" ? "is-holding-shrink" : ""} ${holdingKey === gameViewKey(game) && launchEffect === "freeze" ? "is-holding-freeze" : ""}`}
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
        onTransitionEnd={event => {
          const card = event.currentTarget;
          if (event.target !== card || event.propertyName !== "transform" || card.matches(":hover")) return;
          // An identity perspective/scale3d still retains a GPU surface. Release
          // it once the tilt settles, rather than accumulating one per visited card.
          card.style.transition = "box-shadow .35s ease, border-color .35s ease";
          card.style.transform = "none";
        }}
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
      {holdingKey === gameViewKey(game) && launchEffect === "smooth" && <CoverHoldProgress />}
          {holdingKey === gameViewKey(game) && launchEffect === "freeze" && <CoverFreeze reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "aquarium" && <CoverWaterFill reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "particles" && <CoverHoldParticles reducedMotion={performanceMode} />}
          {holdingKey === gameViewKey(game) && launchEffect === "penguin" && <CoverTuxRise reducedMotion={performanceMode} />}
      {holdingKey === gameViewKey(game) && launchEffect === "shatter" && !isRunning(game) && <div className="library-hold-launch" aria-hidden="true"><svg className="library-hold-cracks" viewBox="0 0 100 100" preserveAspectRatio="none">{coverCrackPaths.map((path, index) => <g key={path}><path className="cover-crack-shadow" d={path} pathLength={1} style={{ animationDelay: `${index / coverCrackPaths.length * .52}s` }} /><path d={path} pathLength={1} style={{ animationDelay: `${index / coverCrackPaths.length * .52}s` }} /></g>)}</svg></div>}

      {runningBadge(game)}
      <div className="glare" style={{
        position: "absolute", top: 0, left: 0, right: 0, bottom: 0,
        pointerEvents: "none", opacity: 0, transition: "opacity .3s ease-out", zIndex: 10
      }} />
      {/* The portrait animates between grid and list views; never the full list row.
          No layoutId: a shared id made the cover, remounted after the details
          closed, animate in from where it was left (tilted, mid-hover) and blink. */}
      <motion.div className="library-cover-thumbnail" layout={performanceMode ? false : "preserve-aspect"} layoutDependency={viewMode}
        onContextMenu={(e) => { e.preventDefault(); setContextMenu({ x: e.clientX, y: e.clientY, game }); }}>
      {!imageErrors.has(game.path) && game.cover_url ? (
        <SmoothImage
          src={game.cover_url}
          alt={game.name}
          loading="lazy"
          decoding="async"
          onLoad={event => { if (!performanceMode && game.cover_url) warmAmbientCover(game.cover_url, event.currentTarget); }}
          style={{ width: "100%", aspectRatio: "2/3", objectFit: "cover", display: "block" }} 
          onError={async () => {
            setImageErrors(prev => new Set(prev).add(game.path));
            if (!hasCustomCover(game)) forgetAutomaticCover(game);
            const launcherFallback = launcherCoverFallback(game);
            if (launcherFallback && game.cover_url !== launcherFallback && !hasCustomCover(game)) {
              setGames(previous => previous.map(item => item.path === game.path ? {...item, cover_url:launcherFallback, automatic_cover_url:launcherFallback} : item));
              setImageErrors(previous => { const next=new Set(previous); next.delete(game.path); return next; });
              return;
            }
            if (!hasCustomCover(game) && !fallbackAttempted.has(game.path)) {
              setFallbackAttempted(prev => new Set(prev).add(game.path));
              try {
                if (managedLibrary?.onCoverFallback) { await managedLibrary.onCoverFallback(game); return; }
                const newCover = await invoke<string | null>("fetch_steamgriddb_cover_command", { name: game.name, apiKey: STEAMGRIDDB_API_KEY });
                if (newCover && !hasCustomCover(game)) {
                  const savedCover = await rememberAutomaticCover(game, newCover);
                  if (hasCustomCover(game)) return;
                  setGames(prev => prev.map(g => g.path === game.path ? { ...g, cover_url: savedCover, automatic_cover_url: savedCover } : g));
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
      
      <GamePlatformBadge game={game} />
      </motion.div>
      <>
        <span className="modern-card-store"><LauncherIcon launcher={game.launcher} size={12} />{game.library_category === "launchers" ? "Launcher" : game.launcher}</span>
        <button type="button" className={`modern-card-fav${favorites.has(gameViewKey(game)) ? " on" : ""}`} aria-pressed={favorites.has(gameViewKey(game))}
          aria-label={t("gameGrid", favorites.has(gameViewKey(game)) ? "unfavorite" : "favorite")} title={t("gameGrid", favorites.has(gameViewKey(game)) ? "unfavorite" : "favorite")}
          onPointerDown={event => event.stopPropagation()} onClick={event => { event.stopPropagation(); toggleFavorite(game); }}><MenuIcon name="heart" /></button>
        {launchable(game) && <button type="button" className={`modern-card-play${isRunning(game) ? " running" : ""}`} disabled={!isRunning(game) && coverActionsBlocked}
          aria-label={isRunning(game) ? t("gameExecution", "cancel") : t("gameExecution", "start")} title={isRunning(game) ? t("gameExecution", "cancel") : t("gameExecution", "start")}
          onPointerDown={event => event.stopPropagation()} onClick={event => { event.stopPropagation(); void onCoverAction(isRunning(game) ? "stop" : "start", game); }}>
          <MenuIcon name={isRunning(game) ? "stop" : "play"} /></button>}
      </>
      <div className="game-list-details"><span>{game.name}</span></div>
    </div>
  );
  };

  // Keep library cells in their grid slots. The portrait still animates into details.
  const renderCard = (game: GameInfo) => (
    <motion.div key={gameViewKey(game)} className={`library-card-cell ${brokenCards.has(gameViewKey(game)) ? "library-card-cell--shattered" : ""}`} aria-hidden={brokenCards.has(gameViewKey(game)) || undefined} data-cover-key={gameViewKey(game)}>
      {renderCardContent(game)}
      {!brokenCards.has(gameViewKey(game)) && <div className="modern-card-caption">
        <p>{game.name}</p>
        <small>{game.nexus_running || game.path === runningGamePath ? t("gameExecution", "running") : playedLabel(game) || (game.launcher === "PeliGames" && game.executable ? game.executable.split(/[\\/]/).pop() : game.library_category === "launchers" ? "Launcher" : game.launcher)}</small>
      </div>}
    </motion.div>
  );

  const cacheScope = managedLibrary ? "nexus" : libraryScope;
  // null means no successful scan yet; an empty saved library is still valid.
  const hasLibraryCache = librarySnapshots.current[cacheScope] !== null;
  const refreshingLibrary = managedLibrary
    ? !managedLibrary.loaded || Boolean(managedLibrary.scanning)
    : scanPending || loading || !libraryLoaded || !ownLoaded;
  const liveGames = managedLibrary ? managedLibrary.games
    : libraryScope === "own" ? games.filter(game => game.launcher === "PeliGames" || Boolean(game.external_id)) : games;
  // Keep the last complete snapshot while scanning; publish the next complete
  // library together so partial discoveries do not shuffle the visible grid.
  const previousGames = librarySnapshots.current[cacheScope] ?? [];
  const displayedGames = refreshingLibrary && hasLibraryCache ? previousGames : liveGames;
  useEffect(() => {
    if (refreshingLibrary || libraryError || managedLibrary?.error) return;
    librarySnapshots.current[cacheScope] = liveGames;
    saveGameLibrary(liveGames, cacheScope);
  }, [cacheScope, refreshingLibrary, games, managedLibrary?.games, managedLibrary?.error, libraryError]);
  const scopedGames = applyCustomCovers(displayedGames
    .map(game => ({ ...game, library_view: cacheScope })))
    .filter(game => !removedGames.has(gameViewKey(game)));
  // Sort the shared presentation before splitting categories and favourites.
  // filter creates a new array, so scanner results and saved libraries stay intact.
  const filteredGames = scopedGames.filter(g => g.name.toLowerCase().includes(searchQuery.toLowerCase()))
    .sort((a, b) => {
      const name = libraryNameOrder.compare(a.name, b.name);
      if (name) return name;
      // Equal titles (including case/accent variants) keep the same order even
      // when different scanners return their results in a different sequence.
      const left = gameViewKey(a), right = gameViewKey(b);
      return left < right ? -1 : left > right ? 1 : 0;
    });
  const favoriteGames = filteredGames.filter(game => favorites.has(gameViewKey(game)));
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
  const categoryOf = (game: GameInfo) => {
    if (game.launcher === "Manual") {
      const folder = customFolders.find(f => game.path.startsWith(f));
      return folder ? folder.split(/[\\/]/).pop() || folder : "Avulsos";
    }
    return game.library_category === "launchers" ? "Launchers" : game.launcher;
  };
  const storeChips = [...new Set(filteredGames.map(categoryOf))].sort((a, b) => libraryNameOrder.compare(a, b));
  const modernGames = filteredGames.filter(game => storeFilter === "all" ? true : storeFilter === "favorites" ? favorites.has(gameViewKey(game)) : categoryOf(game) === storeFilter);
  useEffect(() => { if (!managedLibrary) onCountChange?.(scopedGames.length); }, [Boolean(managedLibrary), scopedGames.length]); // eslint-disable-line react-hooks/exhaustive-deps
  const recentGames = storeFilter === "all" && !searchQuery
    ? filteredGames.filter(game => (activityOf(activity, game)?.last_played ?? 0) > 0)
      .sort((a, b) => (activityOf(activity, b)?.last_played ?? 0) - (activityOf(activity, a)?.last_played ?? 0)).slice(0, 4)
    : [];
  const launchable = (game: GameInfo) => managed || Boolean(game.external_id) || game.launcher === "Steam" || game.launcher === "PeliGames";
  const chipIcon = (category: string) => category === "Launchers" ? <MenuIcon name="gamepad" /> : ["Steam", "Heroic", "PeliGames"].includes(category) ? <LauncherIcon launcher={category} size={14} /> : <MenuIcon name="folder" />;
  const showEmptyOwnLibrary = !scanning && ownLoaded && (libraryScope === "own" || mode === "mods") && !searchQuery && filteredGames.length === 0;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <AnimatePresence>{teleport && <CoverLaunchTeleport value={teleport} reducedMotion={performanceMode} cancellingDisabled={launchCommitted} onCancel={() => cancelPendingLaunch.current?.()} />}</AnimatePresence>
      <div className="modern-lib-toolbar">
        <label className="modern-lib-search">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>
          <input ref={searchInput} value={searchQuery} onChange={event => setSearchQuery(event.target.value)} placeholder={t("modern", "searchLibrary")} aria-label={t("gameGrid", "search")}
            onKeyDown={event => { if (event.key === "Escape") setSearchQuery(""); }} />
          {searchQuery ? <button type="button" aria-label={t("gameGrid", "clearSearch")} onClick={() => { setSearchQuery(""); searchInput.current?.focus(); }}>×</button> : <kbd>Ctrl K</kbd>}
        </label>
        <ModernLibraryFilters>
          <button type="button" aria-pressed={storeFilter === "all"} onClick={() => setStoreFilter("all")}>{t("modern", "all")} <b>{filteredGames.length}</b></button>
          {favoriteGames.length > 0 && <button type="button" aria-pressed={storeFilter === "favorites"} onClick={() => setStoreFilter("favorites")}><i className="modern-chip-dot" />{t("gameGrid", "favorites")} <b>{favoriteGames.length}</b></button>}
          {storeChips.map(category => <button type="button" key={category} aria-pressed={storeFilter === category} onClick={() => setStoreFilter(category)}>{chipIcon(category)}{category} <b>{filteredGames.filter(game => categoryOf(game) === category).length}</b></button>)}
        </ModernLibraryFilters>
        <div className="modern-lib-actions">
          {managedLibrary && <button type="button" className="modern-lib-icon" onClick={managedLibrary.onAccount} title={managedLibrary.accountLabel} aria-label={managedLibrary.accountLabel}><img src="/nexus-mods.svg" alt="" /></button>}
          {!managedLibrary && libraryScope === "all" && <button type="button" className="modern-lib-icon" onClick={() => void handleAddCustomFolder()} title={t("gameGrid", "addFolder")} aria-label={t("gameGrid", "addFolder")}><MenuIcon name="folder" /></button>}
          <button type="button" className={`modern-lib-icon${scanSpinning ? " spinning" : ""}`} disabled={scanSpinning || managedLibrary?.busy} onClick={() => void fetchGames()} title={t("gameGrid", "scanLibrary")} aria-label={t("gameGrid", "scanLibrary")}><MenuIcon name="repair" /></button>
          <div className="modern-lib-view" role="group" aria-label={t("gameGrid", "viewMode")}>
            <button type="button" aria-pressed={viewMode === "grid"} aria-label={t("gameGrid", "gridView")} title={t("gameGrid", "gridView")} onClick={() => setViewMode("grid")}><svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="3" y="3" width="8" height="8" rx="2"/><rect x="13" y="3" width="8" height="8" rx="2"/><rect x="3" y="13" width="8" height="8" rx="2"/><rect x="13" y="13" width="8" height="8" rx="2"/></svg></button>
            <button type="button" aria-pressed={viewMode === "list"} aria-label={t("gameGrid", "listView")} title={t("gameGrid", "listView")} onClick={() => setViewMode("list")}><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true"><path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01"/></svg></button>
          </div>
          <button type="button" className="modern-lib-button primary" disabled={modeDisabled || Boolean(managedLibrary?.busy)} onClick={() => onModeChange("install")} title={t("modern", "addGame")}><MenuIcon name="plusCircle" />{t("modern", "add")}</button>
        </div>
      </div>
      {(libraryError || managedLibrary?.error) && <TimedFeedback>{libraryError || managedLibrary?.error}</TimedFeedback>}

      <motion.div layoutScroll className={`game-grid-container game-library--${viewMode} modern-grid-container`} style={{ flex: 1, minHeight: 0, overflowY: "auto", marginTop: "1rem" }}>
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
        {recentGames.length > 0 && <section className="modern-continue">
          <div className="modern-section-title"><h2>{t("modern", "continuePlaying")}</h2><small>{t("modern", "recent")}</small></div>
          <div key={cacheScope} className="modern-continue-row modern-library-enter">{recentGames.map(game => {
            const cover = !imageErrors.has(game.path) ? game.cover_url : undefined;
            return <div key={gameViewKey(game)} role="button" tabIndex={0} className="modern-continue-card"
              onClick={() => onSelectGame(game, document.querySelector<HTMLElement>(`[data-cover-key="${CSS.escape(gameViewKey(game))}"] .library-cover-thumbnail`) ?? undefined)}
              onKeyDown={event => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); event.currentTarget.click(); } }}>
              <GameBannerImage game={game} cover={cover} />
              <GamePlatformBadge game={game} />
              {launchable(game) && <button type="button" className={`modern-continue-play${isRunning(game) ? " running" : ""}`} disabled={!isRunning(game) && coverActionsBlocked}
                aria-label={isRunning(game) ? t("gameExecution", "cancel") : t("gameExecution", "start")} title={isRunning(game) ? t("gameExecution", "cancel") : t("gameExecution", "start")}
                onClick={event => { event.stopPropagation(); void onCoverAction(isRunning(game) ? "stop" : "start", game); }}><MenuIcon name={isRunning(game) ? "stop" : "play"} /></button>}
              <div className="modern-continue-text"><strong>{game.name}</strong><small><LauncherIcon launcher={game.launcher} size={12} />{isRunning(game) ? t("gameExecution", "running") : playedLabel(game)}</small></div>
            </div>;
          })}</div>
        </section>}
        {filteredGames.length > 0 && <section className="modern-all-games">
          <div className="modern-section-title"><h2>{storeFilter === "all" ? t("modern", "allGames") : storeFilter === "favorites" ? t("gameGrid", "favorites") : storeFilter}</h2><small>{t("modern", "gamesCount").replace("{n}", String(modernGames.length))}</small></div>
          <div key={cacheScope} className={`game-collection game-collection--${viewMode} modern-library-enter`}>{modernGames.map(game => renderCard(game))}</div>
        </section>}

        </>}
      </motion.div>
      {contextMenu && createPortal(<GameCoverMenu game={contextMenu.game} x={contextMenu.x} y={contextMenu.y}
        running={contextMenu.game.path === runningGamePath || Boolean(managedLibrary?.games.find(game => game.nexus_id && game.nexus_id === contextMenu.game.nexus_id)?.nexus_running)}
        preparing={contextMenu.game.path === startingGamePath || Boolean(managedLibrary?.games.find(game => game.nexus_id && game.nexus_id === contextMenu.game.nexus_id)?.nexus_starting)}
        blocked={coverActionsBlocked} onClose={() => setContextMenu(null)}
        onAction={(action, game) => { if (action === "scan") void fetchGames(); else onCoverAction(action, game); }} />, document.body)}

    </div>
  );
};

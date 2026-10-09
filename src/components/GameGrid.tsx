import { NexusLibraryLoading } from "../modules/nexus/NexusLibraryLoading";
import { useFavorites } from "../services/favorites";
import { CoverGameActions, type CoverGameAction } from "./CoverGameActions";
import { gameViewKey } from "../services/gameIdentity";
import { TimedFeedback } from "./TimedFeedback";
import { MenuIcon } from "./MenuIcon";
import { CoverContextMenuPanel, SteamGridCoverHint } from "./CoverContextMenu";
import { STEAMGRIDDB_API_KEY } from "../services/coverConfig";
import { changeLocalCover, resetGameCover, reportCoverError, hasCustomCover } from "../services/customCovers";
import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n/I18nContext";
import { motion } from "framer-motion";
import { usePerformanceMode } from "./EffectsContext";
import { applyCustomCovers, readGameLibrary, saveGameLibrary } from "../services/gameLibraryCache";
import { warmAmbientCover } from "../services/ambientCoverCache";
import { GameEntryButton, GameModeSelector, type GamePanelMode } from "./GameModeSelector";
import { HoverTooltip } from "./HoverTooltip";
import { LauncherIcon } from "./LauncherIcon";
import { openDirectoryPicker } from "../services/tauriService";
import { listPeliGamesEntries } from "../services/installedLibrary";

export interface GameInfo {
  discovery_source?: string | null;
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
  onAdd: () => void; onRefresh: () => void;
  downloadsAction: React.ReactNode; accountLabel: string; onAccount: () => void;
  onCoverFallback?: (game: GameInfo) => Promise<void>;
}
interface Props {
  managedLibrary?: ManagedGameLibrary;
  onCoverAction: (action: CoverGameAction, game: GameInfo) => void;
  coverActionsBlocked?: boolean;
  homeRevision: number;
  onHome: () => void;
  onSelectGame: (game: GameInfo, portrait?: HTMLElement) => void;
  selectedGameKey?: string;
  mode: GamePanelMode;
  detailsOpen: boolean;
  libraryScope: "own" | "all";
  modeDisabled?: boolean;
  onModeChange: (mode: GamePanelMode) => void;
}

export const GameGrid: React.FC<Props> = ({ onCoverAction, coverActionsBlocked, homeRevision, onHome, onSelectGame, selectedGameKey, mode, libraryScope, detailsOpen, onModeChange, modeDisabled, managedLibrary }) => {
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
  const [favoritesCollapsed, setFavoritesCollapsed] = useState(false);
  const performanceMode = usePerformanceMode();
  const [cachedGames] = useState(readGameLibrary);
  const [games, setGames] = useState<GameInfo[]>(cachedGames ?? []);
  const [libraryLoaded, setLibraryLoaded] = useState(cachedGames !== null);
  const startupHandled = useRef(false);
  const scanInProgress = useRef(false);
  const pendingScanFolders = useRef<string[] | null>(null);
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
    // Pelinstall can register an installation from another process.
    window.addEventListener("focus", refresh);
    return () => { ++ownRequest.current; window.removeEventListener("peligamesLibraryChanged", refresh); window.removeEventListener("focus", refresh); };
  }, []);
  useEffect(() => {
    if (!managedLibrary && libraryLoaded) saveGameLibrary(games);
  }, [games, libraryLoaded]);
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

  const fetchGames = async (folders: string[] = customFolders) => {
    if (managedLibrary) { managedLibrary.onRefresh(); return; }
    if (scanInProgress.current) {
      pendingScanFolders.current = folders;
      return;
    }
    scanInProgress.current = true;
    setLoading(true);
    setImageErrors(new Set());
    setFallbackAttempted(new Set());
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
      if (pending) void fetchGames(pending);
      else setLoading(false);
    }
  };

  const handleAddCustomFolder = async () => {
    const dir = await openDirectoryPicker(t("gameGrid", "selectFolder"));
    if (dir && !customFolders.includes(dir)) {
      const newFolders = [...customFolders, dir];
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
      setGames(previous => previous.map(item => item.path === game.path ? { ...item, cover_url: game.cover_url, automatic_cover_url: game.automatic_cover_url } : item));
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

  const renderCardContent = (game: GameInfo) => {
    if (gameViewKey(game) === selectedGameKey) {
      return (
        <div 
          key={gameViewKey(game)}
          className={`game-card-placeholder ${viewMode === "list" ? "game-card-placeholder--list" : ""}`}
          style={{
            background: "rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)",
            border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.12)",
            borderRadius: "8px",
            aspectRatio: "2/3",
            width: "100%",
            opacity: 0.5
          }}
        >
          {game.cover_url && !imageErrors.has(game.path) ? (
            <img className="selected-cover-copy" src={game.cover_url} alt="" aria-hidden="true"
              onError={() => setImageErrors(prev => new Set(prev).add(game.path))} />
          ) : (
            <div className="selected-cover-copy selected-cover-copy--fallback" aria-hidden="true">
              <img src="/peligames.png" alt="" />
              {viewMode === "grid" && <span>{game.name}</span>}
            </div>
          )}
          {viewMode === "list" && <><span className="selected-game-placeholder-name">{game.name}</span><span className="selected-game-list-note">{t("gameGrid", "selectedGame")}</span></>}
        </div>
      );
    }

    return (
      <div
        className={`library-game-card ${viewMode === "list" ? "library-game-card--list" : ""}`}
        key={`${gameViewKey(game)}-${viewMode}-${performanceMode ? "performance" : "elegant"}`} 
        onClick={event => onSelectGame(game, event.currentTarget.querySelector<HTMLElement>(".library-cover-thumbnail") ?? undefined)}
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
      <div className="glare" style={{
        position: "absolute", top: 0, left: 0, right: 0, bottom: 0,
        pointerEvents: "none", opacity: 0, transition: "opacity 0.2s ease-out", zIndex: 10
      }} />
      {/* The same portrait owns the shared layout in both views; never the full list row. */}
      <motion.div className="library-cover-thumbnail" layout={performanceMode ? false : "preserve-aspect"}
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
            if (!hasCustomCover(game.path) && !fallbackAttempted.has(game.path)) {
              setFallbackAttempted(prev => new Set(prev).add(game.path));
              try {
                if (managedLibrary?.onCoverFallback) { await managedLibrary.onCoverFallback(game); return; }
                const newCover = await invoke<string | null>("fetch_steamgriddb_cover_command", { name: game.name, apiKey: STEAMGRIDDB_API_KEY });
                if (newCover && !hasCustomCover(game.path)) {
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
        <img src="/peligames.png" alt="Icon" style={{ position: "absolute", top: "50%", left: "50%", transform: "translate(-50%, -50%)", width: "48px", height: "48px", opacity: 0.5 }} />
      </div>
      
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
    <motion.div key={gameViewKey(game)} className="library-card-cell" layout={performanceMode ? false : "position"}
      transition={{ layout: { duration: 0.3, ease: [0.4, 0, 0.2, 1] } }}>
      {renderCardContent(game)}
    </motion.div>
  );

  const scopedGames = managedLibrary ? applyCustomCovers(managedLibrary.games).map(game => ({ ...game, library_view: "nexus" as const })) : (libraryScope === "own" ? games.filter(game => game.launcher === "PeliGames") : games)
    .map(game => ({ ...game, library_view: libraryScope }));
  const filteredGames = scopedGames.filter(g => g.name.toLowerCase().includes(searchQuery.toLowerCase()));
  const favoriteGames = filteredGames.filter(game => favorites.has(gameViewKey(game)));
  const remainingGames = filteredGames.filter(game => !favorites.has(gameViewKey(game)));
  const steamGames = remainingGames.filter(g => g.launcher === "Steam");
  const otherGames = remainingGames.filter(g => g.launcher !== "Steam");
  const scanning = managedLibrary ? managedLibrary.busy || !managedLibrary.loaded : loading || (libraryScope === "own" && !ownLoaded);
  const showEmptyOwnLibrary = !scanning && ownLoaded && (libraryScope === "own" || mode === "mods") && !searchQuery && filteredGames.length === 0;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
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
              disabled={loading || managedLibrary?.busy}
              style={{ background: "transparent", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)", color: loading ? "white" : "var(--tone-94a3b8, #94a3b8)", cursor: loading ? "not-allowed" : "pointer", padding: "0.4rem 0.6rem", borderRadius: "6px", display: "flex", alignItems: "center", gap: "0.4rem", transition: "all 0.2s", opacity: loading ? 0.7 : 1 }}
              onMouseOver={(e) => { if (!loading) { e.currentTarget.style.background = "rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)"; e.currentTarget.style.color = "var(--tone-f8fafc, #f8fafc)"; } }}
              onMouseOut={(e) => { if (!loading) { e.currentTarget.style.background = "transparent"; e.currentTarget.style.color = "var(--tone-94a3b8, #94a3b8)"; } }}
            >
              <motion.div animate={{ rotate: loading && !performanceMode ? 360 : 0 }} transition={{ repeat: loading && !performanceMode ? Infinity : 0, duration: 1, ease: "linear" }} style={{ display: "flex" }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/></svg>
              </motion.div>
              <span style={{ fontSize: "0.85rem", fontWeight: 500 }}>{loading ? "..." : t("gameGrid", "scan")}</span>
            </button></HoverTooltip>
          </div>

          {(managedLibrary || libraryScope === "all") && <HoverTooltip text={t("gameModes", "goHome")}><button type="button" className="library-mode-button library-home-button" disabled={modeDisabled || managedLibrary?.busy} onClick={onHome}><img src="/peligames.png" alt="" /><span>{t("gameModes", "home")}</span></button></HoverTooltip>}
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
        {scanning ? <div className="empty-library-state"><NexusLibraryLoading reducedMotion={performanceMode} /></div> : <>
        {filteredGames.length === 0 && (managedLibrary ? <div className="empty-library-state">
          {managedLibrary.busy || !managedLibrary.loaded ? <NexusLibraryLoading reducedMotion={performanceMode} />
            : searchQuery ? <p role="status">{t("gameGrid", "noSearchResults")}</p>
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
            {loading || (libraryScope === "own" && !ownLoaded) ? <><p>{t("app", "scanningGames")}</p><div className="loading-spinner" style={{ margin: "1rem auto" }} /></>
              : <p>{searchQuery ? t("gameGrid", "noSearchResults") : libraryScope === "own" ? t("gameGrid", "noOwnGames") : t("app", "noGamesFound")}</p>}
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
      {contextMenu && (
      <CoverContextMenuPanel onClose={() => setContextMenu(null)} x={contextMenu.x} y={contextMenu.y}>
        <CoverGameActions game={contextMenu.game} blocked={coverActionsBlocked} onAction={(action, game) => { setContextMenu(null); onCoverAction(action, game); }} />
        <SteamGridCoverHint />
          <button
            onClick={async () => {
              const game = contextMenu.game;
              setContextMenu(null);
              try { await changeLocalCover(game); } catch (error) { reportCoverError(error); }
            }}
            style={{
              background: "transparent", border: "none", color: "var(--tone-e2e8f0, #e2e8f0)", padding: "0.5rem 1rem",
              textAlign: "left", cursor: "pointer", fontSize: "0.9rem"
            }}
            onMouseOver={(e) => e.currentTarget.style.background = "rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)"}
            onMouseOut={(e) => e.currentTarget.style.background = "transparent"}
          >
            <MenuIcon name="edit" />{t("gameGrid", "changeCover")}
          </button>
          
          <button onClick={async () => {
            const game = contextMenu.game;
            setContextMenu(null);
            try { await resetGameCover(game); } catch (error) { reportCoverError(error); }
          }} style={{ background: "transparent", border: "none", color: "var(--tone-e2e8f0, #e2e8f0)", padding: "0.5rem 1rem", textAlign: "left", cursor: "pointer", fontSize: "0.9rem" }}>
            <MenuIcon name="repair" />{t("gameGrid", "resetCover")}
          </button>

          <button
            onClick={() => {
              fetchGames();
              setContextMenu(null);
            }}
            style={{
              background: "transparent", border: "none", color: "var(--tone-60a5fa, #60a5fa)", padding: "0.5rem 1rem",
              textAlign: "left", cursor: "pointer", fontSize: "0.9rem", borderTop: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)"
            }}
            onMouseOver={(e) => e.currentTarget.style.background = "rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)"}
            onMouseOut={(e) => e.currentTarget.style.background = "transparent"}
          >
            <MenuIcon name="search" />{t("gameGrid", "rescan")}
          </button>
        </CoverContextMenuPanel>
      )}
    </div>
  );
};

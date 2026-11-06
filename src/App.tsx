import {LibraryEntryDialogs} from "./modules/launcher/LibraryEntryDialogs";
import {useLibraryEntryDialogs} from "./modules/launcher/useLibraryEntryDialogs";
import {useGameMenuShortcut} from "./modules/dlssnr/useGameMenuShortcut";
import {usePreferencesRequests, type PreferencesSection} from "./modules/launcher/usePreferencesRequests";
import {useSelectedGameStatus} from "./modules/launcher/useSelectedGameStatus";
import {useLauncherAppearance} from "./modules/launcher/useLauncherAppearance";
import {useLauncherWelcome} from "./modules/launcher/useLauncherWelcome";
import {useLauncherOpenRequest} from "./modules/launcher/useLauncherOpenRequest";
import {SelectedGameOverview} from "./modules/launcher/SelectedGameOverview";
import {GamePathTile} from "./modules/launcher/GamePathTile";
import {getUpscalerFamilies} from "./modules/launcher/gameInfoPresentation";
import { useNeuralModels } from "./modules/dlssnr/useNeuralModels";
import { PeliRunProfile } from "./components/PeliRunProfile";
import { useDlssInstallation } from "./modules/dlssnr/useDlssInstallation";
import { DlssInstallationDialogs } from "./modules/dlssnr/DlssInstallationDialogs";
import {useDiscordPresenceEvents} from "./services/discordPresence";
import {GameConfigurationChips} from "./components/modern/GameConfigurationChips";
import {CapturePanel} from "./modules/capture/CapturePanel";
import { GameResetPanel } from "./components/GameResetPanel";
import { useRemovedPrefixNotice } from "./components/RemovedPrefixNotice";
import { useGameCopyReference } from "./hooks/useGameCopyReference";
import {GameCopyDialog} from "./components/GameCopyDialog";
import {experimentalFrameGeneration, experimentalDanielNr} from "./services/releaseFeatures";
import { sameGameInstallation } from "./services/integrationGameIdentity";
import { GamePerformancePanel } from "./components/GamePerformancePanel";
import { FrameGeneratorTabs } from "./modules/frameGeneration/FrameGeneratorTabs";
import { GameModsTabs, type GameModsTab } from "./modules/nexus/GameModsTabs";
import { DanielNrTestPanel } from "./modules/neuralRendering/DanielNrTestPanel";
import { NexusGameTab } from "./modules/nexus/NexusGameTab";
import { DropImportDialog } from "./components/DropImportDialog";
import { useLauncherFileDrop } from "./hooks/useLauncherFileDrop";
import { GameBenchmarkView } from "./components/preferences/GameBenchmarkView";
import { GameBenchmarkNavigationContext } from "./components/preferences/GameBenchmarkNavigationContext";
import { requireNexusAccount } from "./modules/nexus/requireNexusAccount";
import { SteamPendingActions } from "./components/SteamPendingActions";
import { useGameBanner } from "./hooks/useGameBanner";
import { openUrl } from "@tauri-apps/plugin-opener";
import { OPEN_DISCORD_PANEL_EVENT, SUPPORT_PROJECT_URL, YOUTUBE_MEMBERSHIP_URL } from "./services/communityLinks";
import { WindowResizeHandles } from "./components/WindowResizeHandles";
import { SelectedGameCover } from "./components/modern/SelectedGameCover";
import { useCoverTilt } from "./hooks/useCoverTilt";
import { listen } from "@tauri-apps/api/event";
import { NexusBrowserPopup } from "./modules/nexus/NexusBrowserPopup";
import { LauncherSettingsMenu } from "./components/LauncherSettingsMenu";
import { type CoverGameAction } from "./components/CoverGameActions";
import { GameLogsModal } from "./components/GameLogsModal";
import { gameViewKey } from "./services/gameIdentity";
import { GameEntryModal } from "./components/GameEntryModal";
import { type UninstallOptions, addPeliGamesEntry, uninstallPeliGamesEntry, listPeliGamesEntries, choosePeliGamesEntry, listPeliGamesExecutableChoices } from "./services/installedLibrary";
import { notifyError, notifyInfo } from "./services/notifications";
import { useGameExecution } from "./hooks/useGameExecution";
import { useTitlebarRunningGames } from "./hooks/useTitlebarRunningGames";
import { HoverTooltip } from "./components/HoverTooltip";
import { ModernQuickIcon } from "./components/modern/ModernQuickIcon";
import { GameRunTimer, useRunningGameTimes } from "./components/GameRunTimer";
import { useMatrixGameActivity } from "./services/matrixGameActivity";
import { InstalledGamePanels, type InstalledPanelSection } from "./components/InstalledGamePanels";
import { useInstallationLocation } from "./hooks/useInstallationLocation";
import { useGameInstaller } from "./hooks/useGameInstaller";
import { useInstallationGameDraft } from "./hooks/useInstallationGameDraft";
import { useNeuralStartup } from "./hooks/useNeuralStartup";
import { isInstalledMod } from "./services/installationStatus";
import { GameCoverMenu } from "./components/GameCoverMenu";
import { canRemoveFromLibrary, setLibraryGameRemoved } from "./services/libraryVisibility";
import { cloneElement, useState, useEffect, useLayoutEffect, useRef, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GameDetailsOverlay } from "./components/modern/GameDetailsOverlay";
import { useAnimatedDetailsHeight } from "./hooks/useAnimatedDetailsHeight";
import { useCoverFlight } from "./hooks/useCoverFlight";
import { useLibraryTransition } from "./hooks/useLibraryTransition";
import { useGameAnalysis } from "./hooks/useGameAnalysis";
import { useI18n } from "./i18n/I18nContext";
import { AnimatePresence, motion } from "framer-motion";
import "./App.css";
import "./theme/appearance.css";
import "./theme/glassSurfaces.css";
import "./theme/crystalActions.css";
import { MenuIcon } from "./components/MenuIcon";
import type { GamePanelMode } from "./types/gamePanel";
import { InstallGameSummary, InstallGamePanels, InstallGameSubmit } from "./components/InstallGameView";

// Types
import { InstallRoute } from "./types/installer";

// Services
import { openDirectoryPicker } from "./services/tauriService";
import { OPEN_MATRIX_CHAT_EVENT } from "./services/matrixChat";

// Components
import { LauncherUpdateIndicator } from "./components/LauncherUpdateIndicator";
import { useLauncherUpdate } from "./modules/launcherUpdates/useLauncherUpdate";
import { AppUpdateModal } from "./components/AppUpdateModal";
import { BackendUpdaterModal } from "./components/BackendUpdaterModal";
import { CreditsModal } from "./components/CreditsModal";
import { NotificationCenter } from "./components/NotificationCenter";
import { RuntimeActivityNotice } from "./components/RuntimeActivityNotice";
import { TitleBar } from "./components/TitleBar";
import {SetupWizard} from "./components/SetupWizard";
import type {AppConfig} from "./types/appConfig";
import {useAppConfig} from "./modules/appConfig/useAppConfig";
import { SettingsModal, type SettingsPage } from "./components/SettingsModal";
import { useFavorites, toggleFavorite } from "./services/favorites";
import { FeatureReportSection } from "./modules/reports/FeatureReportSection";
import { featureReportContext, type ReportCategory } from "./modules/reports/reportContext";
import { NexusReportModal } from "./modules/nexus/NexusReportModal";
import { NexusLogsModal } from "./modules/nexus/NexusLogsModal";
import { NexusPopup } from "./modules/nexus/NexusPopup";
import { NexusDownloadsPanel } from "./modules/nexus/NexusDownloadsPanel";
import { useNexusGameTools } from "./modules/nexus/useNexusGameTools";
import { NexusPanels } from "./modules/nexus/NexusPanels";
import { NexusCatalogPanel } from "./modules/nexus/NexusCatalogPanel";
import { NexusEmptySummary } from "./modules/nexus/NexusEmptySummary";
import { useNexusWorkspace } from "./modules/nexus/useNexusWorkspace";
import "./modules/nexus/nexus.css";
import "./theme/controlStability.css";
import { ModManagerModal } from "./components/ModManagerModal";
import { WelcomeModal } from "./components/WelcomeModal";
import { AppImageInstallPrompt } from "./components/AppImageInstallPrompt";
import { hasDlssnrConfiguration, modelSourcePaths } from "./services/modConfiguration";
import { GameGrid, GameInfo } from "./components/GameGrid";
import { InstructionsModal } from "./components/InstructionsModal";
import { PrefixUninstallModal } from "./components/PrefixUninstallModal";
import { ConfirmModal } from "./components/ConfirmModal";
import { activityOf, daysSince, formatPlaytime, loadGameActivity, type ActivityMap } from "./services/gameActivity";
import { ModernShell, type ModernSection, type ModernSettingsPage } from "./components/modern/ModernShell";
import { ModernGameDetails, type ModernTab } from "./components/modern/ModernGameDetails";
import { ModernDlssPanel } from "./components/modern/ModernDlssPanel";
import { LauncherIcon } from "./components/LauncherIcon";
import "./components/modern/modern.css";
import { ShortcutKeySelector } from "./components/ShortcutKeySelector";

import { EffectsContext } from "./components/EffectsContext";
import { APP_BUILD_LABEL } from "./services/buildInfo";
import { menuShortcutForRoute } from "./services/routeDefaults";
// Launch report shown over the library (same order as when Pelinstall was bundled here).
import "./modules/pelinstall/pelinstall.css";

function App({ initialGamePath }: { initialGamePath?: string }) {

  const emptyPreview = import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window ? new URLSearchParams(location.search).get("empty") || new URLSearchParams(location.search).get("demo") : null;
  const {performanceMode, setPerformanceMode, playGameEntrance, setPlayGameEntrance, effectsHaveChanged} = useLauncherAppearance();
  const coverFlight = useCoverFlight(performanceMode);
  // New interface: playtime shown in the game spotlight.
  const [gameActivity, setGameActivity] = useState<ActivityMap>({});
  // New interface: Settings is a page of the window, picked from the side menu.
  const [settingsPage, setSettingsPage] = useState<SettingsPage>("home");
  const [libraryCount, setLibraryCount] = useState(0);
  const favorites = useFavorites();
  const [downloadsRequest, setDownloadsRequest] = useState(0), [activeDownloads, setActiveDownloads] = useState(0);
  useLauncherOpenRequest(initialGamePath, entry => {
    setShowWelcome(false);
    setSelectedGame(entry); setGamePanelMode("installed"); setLibraryScope("own"); setGameDir(entry.directory || entry.path);
  });
  const [gameDir, setGameDir] = useState("");
  const [route, setRoute] = useState<InstallRoute>("optiscaler");
  const [bitness, setBitness] = useState<"32" | "64">("64");
  const [gpuArch, setGpuArch] = useState<"rdna4" | "rdna3">("rdna4");
  const { t, language } = useI18n();

  const modInstallation = useDlssInstallation();

  const launcherUpdate = useLauncherUpdate();
  const [showUpdaterModal, setShowUpdaterModal] = useState(false);
  const [showInstructionsModal, setShowInstructionsModal] = useState(false);
  const [showCreditsModal, setShowCreditsModal] = useState(false);
  const [showSetupWizard, setShowSetupWizard] = useState(false);
  const {showWelcome, setShowWelcome, appImageTarget, setAppImageTarget} = useLauncherWelcome(initialGamePath);
  const [settingsSection, setSettingsSection] = useState<PreferencesSection>("general");
  const [preferencesBusy, setPreferencesBusy] = useState(false);
  const [notificationBackendArch, setNotificationBackendArch] = useState<"rdna3" | "rdna4">("rdna4");
  const [showSettingsModal, setShowSettingsModal] = useState(false);
  const preferences = useAppConfig();
  const appConfig = preferences.config;
  const changeMenuShortcut = useGameMenuShortcut();
  const appConfigLoaded = preferences.loaded;

  const changeInstallRoute = (nextRoute: InstallRoute) => {
    if (nextRoute === route) return;
    setRoute(nextRoute);
  };

  const [showSettings, setShowSettings] = useState(false);
  const [directoryPickerBusy, setDirectoryPickerBusy] = useState(false);
  const coverContextMenuRef = useRef<HTMLDivElement>(null);
  const [copyTarget,setCopyTarget]=useState<GameInfo|null>(null);
  const [removeLibraryTarget, setRemoveLibraryTarget] = useState<GameInfo | null>(null);
  const [gameLogsTarget, setGameLogsTarget] = useState<GameInfo | null>(null);
  const [coverContextMenu, setCoverContextMenu] = useState<{ x: number, y: number } | null>(null);

  useEffect(() => {
    if (appConfig) {
      setGpuArch(appConfig.backend === "AMDNR" ? "rdna4" : "rdna3");
    }
  }, [appConfig]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (!coverContextMenuRef.current?.contains(event.target as Node)) setCoverContextMenu(null);
    };
    if (coverContextMenu) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [coverContextMenu]);

  useEffect(() => {
    if (bitness === "32" && route === "optiscaler") {
      changeInstallRoute("reshade"); // Fallback to reshade
    }
  }, [bitness, route]);

  const modOperationContext = (path: string) => ({
    game: selectedGame && (selectedGameDirectory === path || selectedGame.path === path) ? selectedGame : null,
    displayName: selectedGame?.name,
    onGameUpdated: (game: GameInfo) => setSelectedGame(previous => previous?.path === game.path ? game : previous),
    onRefresh: async () => {
      if (selectedGame && (selectedGameDirectory === path || selectedGame.path === path)) {
        await refreshInstallationDetails(path);
        setGameInfoRevision(value => value + 1);
      }
    },
  });
  const handleInstall = () => {
    if (gpuArch === "rdna3" && bitness === "32") return;
    if (!gameDir) { setShowInstructionsModal(true); return; }
    const context = modOperationContext(gameDir);
    return modInstallation.install({
      ...context, modelConfigured: hasDlssnrConfiguration(appConfig), installed: isInstalledMod(installStatus),
      onRefresh: async () => { await context.onRefresh(); if (context.game) neuralStartup.refresh(); },
      payload: {
        gameDir, route, bitness, gpuArch,
        ...modelSourcePaths(appConfig),
        shortcutKey: menuShortcutForRoute(appConfig?.game_shortcut_keys, selectedGame?.path || gameDir, gameDir, route),
        neuralStartup: neuralStartup.enabled, model: neuralModel, int4: int4Mixed, renodx: neuralStartup.renodx,
        lumenite: route !== "optiscaler" && Boolean(neuralModels?.lumenite) && (lumeniteChoices[gameDir] ?? neuralModels?.lumenite_selected ?? false),
      },
    });
  };
  const proceedUninstallForPath = (path: string) => modInstallation.uninstall({
    ...modOperationContext(path), payload: { gameDir: path, route: "remove", bitness, gpuArch, dllPath: null, binPath: null },
  });

  const [selectedGame, setSelectedGame] = useState<GameInfo | null>(null);

  const [unifiedTab, setUnifiedTab] = useState("overview");
  const [gameModsTab, setGameModsTab] = useState<GameModsTab>("nexus");
  const nexus = useNexusWorkspace(unifiedTab === "nexus" || unifiedTab === "tools");
  const [addingNexusGame, setAddingNexusGame] = useState(false);
  const [nexusAdditionPrefix, setNexusAdditionPrefix] = useState("");
  const [nexusAdditionPlatform, setNexusAdditionPlatform] = useState<"native" | "proton">("proton");
  const [modManagerOpen, setModManagerOpen] = useState(false);
  const [showModManagerModal, setShowModManagerModal] = useState(false);
  const [nexusModToView, setNexusModToView] = useState<{gameId: string; modId: string; requestId: number} | null>(null);
  const launcherScroll = useRef<HTMLDivElement>(null);
  const transitionLibrary = useLibraryTransition(launcherScroll, performanceMode || showSettingsModal);
  const modViewRequest = useRef(0);
  const [nexusTab, setNexusTab] = useState<"mods" | "catalog">("mods");
  const [gamePanelMode, setGamePanelMode] = useState<GamePanelMode>(emptyPreview?.startsWith("nexus") ? "nexus" : emptyPreview === "dlss" ? "mods" : "install");
  const dlssActive = (unifiedTab === "nexus" && gameModsTab === "dlss") || (gamePanelMode === "mods" && gameModsTab !== "daniel");
  const [modPaths, setModPaths] = useState<{key:string; root:string; executable?:string; dlss_destination?:string}>();
  const [modPathsError, setModPathsError] = useState("");
  const modPathCache = useRef(new Map<string,{root:string;executable?:string;dlss_destination?:string}>());
  const selectedTabIntent = useRef<string | null>(null);
  useEffect(() => {if(selectedGame){setUnifiedTab(selectedTabIntent.current || "overview");selectedTabIntent.current=null;}}, [selectedGame?.path]);
  const { neuralModel, setNeuralModel, neuralModels, neuralModelsError, lumeniteChoices, setLumeniteChoices, int4Mixed, setInt4Mixed } = useNeuralModels({
    active: dlssActive, gamePath: selectedGame?.path, gameDir, config: appConfig, gpuArch, bitness,
    paused: modInstallation.busy || showSetupWizard || showSettingsModal,
  });
  const [executableAlternatives, setExecutableAlternatives] = useState<GameInfo[]>([]);
  useEffect(() => {
    let active = true;
    setExecutableAlternatives([]);
    if (selectedGame?.launcher === "PeliGames" && gamePanelMode === "installed") {
      void listPeliGamesExecutableChoices(selectedGame.path).then(entries => { if (active) setExecutableAlternatives(entries); }).catch(console.error);
    }
    return () => { active = false; };
  }, [selectedGame?.path, gamePanelMode]);

  const [homeRevision, setHomeRevision] = useState(0);
  const [libraryScope, setLibraryScope] = useState<"own" | "all">(emptyPreview && emptyPreview !== "launcher" ? "all" : "own");
  useEffect(() => { if (emptyPreview?.startsWith("nexus")) void nexus.refresh(); }, []);
  const [showGameEntryModal, setShowGameEntryModal] = useState(false);
  const [droppedPath, setDroppedPath] = useState<string | null>(null);
  const [libraryMutationBusy, setLibraryMutationBusy] = useState(false);
  const libraryMutationLock = useRef(false);
  const [libraryMutationError, setLibraryMutationError] = useState("");
  const [uninstallTarget, setUninstallTarget] = useState<{ path: string; name: string; prefix: string } | null>(null);
  const [installationOpen, setInstallationOpen] = useState(false);
  const installationDraft = useInstallationGameDraft(true);
  const installationLocation = useInstallationLocation(installationDraft.name, installationOpen && !addingNexusGame);
  const removedPrefixNotice=useRemovedPrefixNotice(installationLocation.trackReservation);
  const [installationExecutable, setInstallationExecutable] = useState("");
  const [installationProton, setInstallationProton] = useState("");
  const gameInstaller = useGameInstaller();
  const gameExecution = useGameExecution();
  const libraryDialogs = useLibraryEntryDialogs({
    mutation: {busy: libraryMutationBusy, lock: libraryMutationLock, setBusy: setLibraryMutationBusy}, sessions: gameExecution.sessions,
    onSelected: (game, changing) => {
      if (changing) {setSelectedGame(previous => previous ? {...game, library_view: previous.library_view} : previous); setGameDir(game.directory || game.path);}
      else showCompletedGameEntry(game);
    },
  });
  const selectedSession=selectedGame?gameExecution.statusFor(selectedGame.path):undefined;
  const copyReference=useGameCopyReference(selectedGame);
  const reservedBase=Boolean(copyReference?.reserved);
  const sessionState = selectedSession?.state;
  useEffect(() => {
    if (!selectedGame) return;
    let alive = true;
    void loadGameActivity().then(value => { if (alive) setGameActivity(value); });
    return () => { alive = false; };
  }, [selectedGame?.path, sessionState]);
  // Launcher errors also go to the notification center.
  useEffect(() => { if (gameExecution.error) notifyError(t("ui-app", "game_execution"), gameExecution.error); }, [gameExecution.error]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (libraryMutationError) notifyError(t("ui-app", "library"), libraryMutationError); }, [libraryMutationError]); // eslint-disable-line react-hooks/exhaustive-deps
  const installationReady = Boolean(!gameExecution.active && !gameExecution.pendingPath && installationDraft.name && !installationDraft.editing && installationLocation.selected && installationLocation.directory.trim() && installationExecutable.trim() && (addingNexusGame || installationProton));
  const [isCollapsingGame, setIsCollapsingGame] = useState(false);
  const detailsOpen = !isCollapsingGame && ((gamePanelMode === "install" || gamePanelMode === "add") ? installationOpen : gamePanelMode === "installed" ? Boolean(selectedGame) : gamePanelMode === "nexus" ? modManagerOpen && Boolean(selectedGame) : modManagerOpen);
  // Shortcut destinations temporarily unmount details; keep the origin tab here.
  const detailsResetKey = selectedGame ? `game:${selectedGame.external_id || selectedGame.path}` : `${gamePanelMode}:none`;
  const [detailsReturnTab, setDetailsReturnTab] = useState<{ key: string; tab: string } | null>(null);
  const [benchmarkViewKey, setBenchmarkViewKey] = useState<string | null>(null);
  const benchmarkOpen = detailsOpen && benchmarkViewKey === detailsResetKey;
  const benchmarkScroll = useRef(0), previousBenchmarkOpen = useRef(false);
  useEffect(() => { setDetailsReturnTab(null); setBenchmarkViewKey(null); }, [detailsResetKey, detailsOpen]);
  useLayoutEffect(() => {
    if (previousBenchmarkOpen.current === benchmarkOpen) return;
    previousBenchmarkOpen.current = benchmarkOpen;
    const scroll = document.querySelector<HTMLElement>(".game-details-scroll");
    if (scroll) scroll.scrollTop = benchmarkOpen ? 0 : benchmarkScroll.current;
    if (!benchmarkOpen) document.querySelector<HTMLButtonElement>("[data-game-benchmark-open]")?.focus({preventScroll:true});
  }, [benchmarkOpen]);
  const openGameBenchmarks = () => {
    benchmarkScroll.current = document.querySelector<HTMLElement>(".game-details-scroll")?.scrollTop ?? 0;
    setBenchmarkViewKey(detailsResetKey);
  };
  const gameDetailsOverlay = Boolean(selectedGame && !["install", "add"].includes(gamePanelMode));
  const detailsSize = useAnimatedDetailsHeight(detailsOpen && !gameDetailsOverlay ? ((gamePanelMode === "install" || gamePanelMode === "add") ? "installation-draft" : selectedGame?.path ?? "mod-manager-empty") : undefined, performanceMode);
  const [showDiscardDraft, setShowDiscardDraft] = useState(false);
  const pendingDraftExit = useRef<(() => void) | null>(null);
  const hasUnfinishedDraft = installationOpen && (gamePanelMode === "install" || gamePanelMode === "add") && !gameInstaller.result && Boolean(
    installationDraft.name.trim() || installationDraft.draftName.trim() || installationLocation.selected || installationExecutable.trim() || installationProton
  );
  const requestDraftExit = (action: () => void) => {
    if (gameInstaller.busy || libraryMutationBusy || nexus.busy || pendingDraftExit.current) return;
    if (hasUnfinishedDraft) {
      pendingDraftExit.current = action;
      setShowDiscardDraft(true);
    } else action();
  };
  const collapseGameDetailsNow = (deferSelection = false) => {
    if (gameInstaller.busy || libraryMutationBusy) return;
    setNexusTab("mods");
    if (deferSelection && detailsOpen && selectedGame) {
      const cell = Array.from(document.querySelectorAll<HTMLElement>(".library-card-cell"))
        .find(element => element.dataset.coverKey === gameViewKey(selectedGame));
      const target = cell?.querySelector<HTMLElement>(".selected-cover-copy, .library-cover-thumbnail");
      coverFlight.returnTo(target ?? undefined, () => { setSelectedGame(null); setIsCollapsingGame(false); });
    }
    gameInstaller.reset();
    setIsCollapsingGame(true);
    setCoverContextMenu(null);
    // Keep the selected cover mounted until the exit slide finishes.
    if (!deferSelection || !detailsOpen) setSelectedGame(null);
    setInstallationOpen(false);
    setModManagerOpen(false);
    if (addingNexusGame) setGamePanelMode("nexus");
    setAddingNexusGame(false); setNexusAdditionPrefix("");
    installationDraft.reset();
    installationLocation.reset(); setInstallationExecutable(""); setInstallationProton("");
    setGameDir("");
  };
  const collapseGameDetails = () => requestDraftExit(() => collapseGameDetailsNow(true));
  const returnToLauncherHome = () => requestDraftExit(() => transitionLibrary(() => {
    collapseGameDetailsNow();
    setGamePanelMode("install");
    setLibraryScope("own");
    setShowSettings(false);

    setHomeRevision(value => value + 1);
  }, gamePanelMode === "mods" || gamePanelMode === "nexus" || libraryScope !== "own"));
  const changeGamePanelMode = (mode: GamePanelMode) => requestDraftExit(() => {
    if (gameInstaller.busy || libraryMutationBusy) return;
    if (mode === "mods") {
      setShowModManagerModal(true);
      return;
    }
    if (mode === "install") { setShowGameEntryModal(true); return; }
    if (!detailsOpen) setPlayGameEntrance(true);
    setGamePanelMode(mode);
    setLibraryScope("own");
    if (mode === "add") {
      setInstallationOpen(true);
      setIsCollapsingGame(false);
    }
    setCoverContextMenu(null);
  });
  const openModLibrary = (config: AppConfig | null = appConfig) => requestDraftExit(() => {
    if (!hasDlssnrConfiguration(config)) {
      collapseGameDetailsNow();
      setGamePanelMode("install"); setLibraryScope("own");
      setShowSetupWizard(true);
      return;
    }
    transitionLibrary(() => {
      collapseGameDetailsNow();
      setGamePanelMode("installed");
      setLibraryScope("own");
    }, gamePanelMode !== "mods" || libraryScope !== "all");
  });
  useEffect(() => {
    if (!appConfigLoaded || gamePanelMode !== "mods" || hasDlssnrConfiguration(appConfig)) return;
    collapseGameDetailsNow();
    setGamePanelMode("install"); setLibraryScope("own");
    setShowSetupWizard(true);
  }, [appConfigLoaded, gamePanelMode, appConfig?.backend, appConfig?.dll_version]); // eslint-disable-line react-hooks/exhaustive-deps
  const closeDlssSetup = () => {
    setShowSetupWizard(false); setShowModManagerModal(false); setShowSettingsModal(false);
    returnToLauncherHome();
  };
  const openNexusAddition = () => requestDraftExit(() => {
    setShowGameEntryModal(true);
  });
  const openNexusLibrary = () => requestDraftExit(() => transitionLibrary(() => {
    collapseGameDetailsNow();
    setGamePanelMode("installed"); setLibraryScope("own");
    setIsCollapsingGame(false); setPlayGameEntrance(true);
    void nexus.refresh();
  }, gamePanelMode !== "nexus"));
  const nexusIncoming = useRef<(domain: string) => void>(() => {});
  nexusIncoming.current = domain => requestDraftExit(() => {
    void invoke<import("./modules/nexus/useNexusWorkspace").NexusWorkspace>("load_nexus_workspace").then(workspace => {
      const matches = workspace.games.filter(item => item.nexus_domain === domain);
      collapseGameDetailsNow();
      setShowWelcome(false); setGamePanelMode("nexus"); setLibraryScope("all"); setModManagerOpen(matches.length === 1); setIsCollapsingGame(false);
      if (matches.length === 1) { setSelectedGame(matches[0].game); setGameDir(matches[0].game.directory || matches[0].game.path); }
      void nexus.refresh();
    }).catch(error => nexus.setError(String(error)));
  });
  useEffect(() => {
    if ("__PELI_UI_PREVIEW__" in window) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void listen<string>("nexus-download-request", event => { if (active) nexusIncoming.current(event.payload); }).then(stop => {
      if (!active) { stop(); return; }
      unlisten = stop;
      void invoke<import("./modules/nexus/NexusDownloadsPanel").NexusDownload[]>("list_nexus_downloads").then(rows => {
        const recent = rows[rows.length - 1]; if (active && recent) nexusIncoming.current(recent.domain);
      }).catch(() => {});
    });
    return () => { active = false; unlisten?.(); };
  }, []);
  const [nexusReportTarget, setNexusReportTarget] = useState<string | null>(null);
  const [nexusLogTarget, setNexusLogTarget] = useState<string | null>(null);
  const [cachedNexus, setCachedNexus] = useState<import("./modules/nexus/useNexusWorkspace").NexusWorkspace>({ games: [] });
  useEffect(() => {
    if (!selectedGame || gamePanelMode === "mods") return;
    let active = true;
    void invoke<import("./modules/nexus/useNexusWorkspace").NexusWorkspace>("load_nexus_workspace").then(workspace => { if (active) setCachedNexus(workspace); }).catch(() => { if (active) setCachedNexus({ games: [] }); });
    return () => { active = false; };
  }, [selectedGame?.path, gamePanelMode]);
  const nexusCandidates = (nexus.loaded ? nexus.workspace.games : cachedNexus.games).filter(item => selectedGame && (selectedGame.nexus_id ? item.id===selectedGame.nexus_id : sameGameInstallation(item.game,selectedGame)));
  const nexusGame = nexusCandidates.find(item => item.id === selectedGame?.nexus_id || (item.game.path === selectedGame?.path && item.game.launcher === selectedGame?.launcher)) ?? (nexusCandidates.length === 1 ? nexusCandidates[0] : undefined);
  const nexusToolState = useNexusGameTools(unifiedTab === "nexus" || unifiedTab === "tools" || gamePanelMode === "nexus" ? nexusGame : undefined);
  useEffect(() => {
    const coverChanged = (event: Event) => {
      const game = (event as CustomEvent<GameInfo>).detail;
      setSelectedGame(previous => previous && gameViewKey({ ...previous, library_view: gamePanelMode === "nexus" ? "nexus" : libraryScope }) === gameViewKey(game) ? { ...previous, cover_url: game.cover_url, automatic_cover_url: game.automatic_cover_url } : previous);
    };
    const coverError = () => {
      modInstallation.showError(t("gameGrid", "coverErrorTitle"), t("gameGrid", "coverError"));
    };
    window.addEventListener("gameCoverChanged", coverChanged);
    window.addEventListener("gameCoverError", coverError);
    return () => { window.removeEventListener("gameCoverChanged", coverChanged); window.removeEventListener("gameCoverError", coverError); };
  }, [language, gamePanelMode, libraryScope]);
  const [gameInfoRevision, setGameInfoRevision] = useState(0);
  useEffect(() => {
    const refresh = () => setGameInfoRevision(revision => revision + 1);
    window.addEventListener("gameInfoCacheCleared", refresh);
    return () => window.removeEventListener("gameInfoCacheCleared", refresh);
  }, []);
  const registeredGameDirectory = selectedGame?.directory ?? selectedGame?.path;
  const modPathKey = selectedGame ? `${selectedGame.external_id || selectedGame.path}:${registeredGameDirectory}:${selectedGame.executable || ""}:${gameInfoRevision}` : "";
  const selectedGameRoot = nexusGame?.game.directory || (modPaths?.key === modPathKey ? modPaths.root : registeredGameDirectory);
  useEffect(() => {
    if (!dlssActive || !selectedGameRoot) return;
    let alive = true;
    const cached = modPathCache.current.get(modPathKey);
    if (cached) {setModPaths({...cached,key:modPathKey});setGameDir(cached.dlss_destination || selectedGameRoot);return;}
    setModPathsError("");
    void invoke<{root:string;executable?:string;dlss_destination?:string}>("resolve_game_mod_paths", {directory:selectedGameRoot,executable:selectedGame?.executable || null,gameKey:selectedGame?.external_id || selectedGame?.path,legacyOverride:appConfig?.custom_game_paths?.[selectedGame?.name || ""] || null})
      .then(paths=>{if(alive){modPathCache.current.set(modPathKey,paths);setModPaths({...paths,key:modPathKey});setGameDir(paths.dlss_destination || selectedGameRoot);}})
      .catch(e=>{if(alive)setModPathsError(String(e));});
    return()=>{alive=false};
  }, [dlssActive, modPathKey]);
  const selectedGameDirectory = dlssActive && modPaths?.key === modPathKey ? modPaths.dlss_destination || selectedGameRoot : selectedGameRoot;
  const {releaseDate, installStatus, installedDll, dlssNrInstalled, refreshInstallationDetails, prepareSelection} = useSelectedGameStatus(selectedGame, selectedGameDirectory, gameInfoRevision);
  const analysis = useGameAnalysis(dlssActive ? selectedGameDirectory : undefined, selectedGame?.name, selectedGame?.app_id || undefined, gameInfoRevision);
  // New interface: the overview shows the same read-only analysis for every library.
  const overviewAnalysis = useGameAnalysis(selectedGame && !["mods", "install", "add"].includes(gamePanelMode) ? selectedGameDirectory : undefined, selectedGame?.name, selectedGame?.app_id || undefined, gameInfoRevision);
  const upscalerFamilies = getUpscalerFamilies(analysis.upscalers);
  const manualRoutes = useRef(new Map<string, InstallRoute>());
  const detectedBitness = analysis.architecture === "32-bits" ? "32" : analysis.architecture === "64-bits" ? "64" : null;
  useEffect(() => {
    if (detectedBitness) setBitness(detectedBitness);
  }, [selectedGame?.path, detectedBitness, gameInfoRevision]);
  const recommendOptiscaler = detectedBitness === "64" && bitness === "64" && upscalerFamilies.some(name => ["DLSS", "FSR", "XeSS"].includes(name));
  useEffect(() => {
    if (selectedGame && recommendOptiscaler && !manualRoutes.current.has(selectedGame.path)) changeInstallRoute("optiscaler");
  }, [selectedGame?.path, recommendOptiscaler]);
  const neuralStartup = useNeuralStartup(dlssActive ? selectedGameDirectory : undefined, dlssActive && isInstalledMod(installStatus));
  const titlebarRunningGames = useTitlebarRunningGames({execution:gameExecution, selected:selectedGame,
    source:gamePanelMode === "mods" || libraryScope === "all" ? "dlss" : "main",
    nexusGames:nexus.workspace.games, nexusLaunching:nexus.launching, nexusStopping:nexus.stopping, nexusBusy:nexus.busy, stopNexus:nexus.stop, onDetails:(game, origin)=>handleSelectGame(game, undefined, true, origin)});
  const runningGameTimes = useRunningGameTimes(titlebarRunningGames);
  useMatrixGameActivity(titlebarRunningGames);
  useDiscordPresenceEvents();
  const handleSelectGame = (game: GameInfo, portrait?: HTMLElement, forceOpen = false, origin?: "main" | "dlss" | "nexus") => requestDraftExit(() => {
    if (gameInstaller.busy || libraryMutationBusy) return;
    if (!forceOpen && gamePanelMode !== "install" && gamePanelMode !== "add" && selectedGame && gameViewKey(game) === gameViewKey(selectedGame)) {
      collapseGameDetails();
    } else {
      setNexusTab("mods");
      coverFlight.start(portrait);
      if (installationOpen) collapseGameDetailsNow();
      setIsCollapsingGame(false);
      setPlayGameEntrance(true);
      prepareSelection();
      const scope = origin ? origin === "main" ? "own" : "all" : libraryScope;
      if (origin) { setLibraryScope(scope); setShowWelcome(false); setShowSettings(false); }
      const manualRoute = manualRoutes.current.get(game.path);
      if (scope === "all" && manualRoute) changeInstallRoute(manualRoute);
      setModManagerOpen(true);
      setGamePanelMode("installed"); setLibraryScope("own"); setGameModsTab(origin === "dlss" ? "dlss" : "nexus"); selectedTabIntent.current=origin === "nexus" || origin === "dlss" ? "nexus" : "overview"; setUnifiedTab(selectedTabIntent.current);
      setGameDir(game.directory ?? game.path);
      setSelectedGame(game);
    }
  });

  const coverAction = async (action: CoverGameAction, game: GameInfo) => {
    setCoverContextMenu(null);
    if(action==="start"&&game.launcher==="PeliGames"&&!("__PELI_UI_PREVIEW__" in window)){
      try{const relation=await invoke<{reserved:boolean}>("get_game_copy_reference",{path:game.path});if(relation.reserved){notifyError("Base reservada","Esta instalação serve de base para cópias overlay. Execute uma cópia ou remova todos os overlays para liberar a base.");return;}}catch(error){notifyError("Referência da cópia",String(error));return;}
    }
    const modEntry = nexus.workspace.games.find(entry => entry.id === game.nexus_id || entry.game.path === game.path || (entry.game.directory || entry.game.path) === (game.directory || game.path));
    if ((action === "start" || action === "stop") && modEntry && (modEntry.mods.some(m=>m.installed && m.enabled) || modEntry.profile || game.nexus_id)) {
      if(action === "stop")return nexus.stop(modEntry.id);
      return (async()=>{ if(game.advanced && !await nexus.configureAdvanced(modEntry.id,game.advanced))return; await nexus.start(modEntry.id); })();
    }
    if (action === "favorite") return;
    if (action === "scan") { if (game.library_view === "nexus") void nexus.refresh(); else window.dispatchEvent(new Event("rescanGames")); return; }
    if (action === "remove") { if (!gameInstaller.busy && !libraryMutationBusy && !nexus.busy && canRemoveFromLibrary(game)) setRemoveLibraryTarget(game); return; }
    if (action === "details") { handleSelectGame(game, undefined, true); return; }
    if (action === "logs") { if (game.library_view === "nexus") { const entry = nexus.workspace.games.find(item => item.game.path === game.path && item.game.launcher === game.launcher); if (entry) setNexusLogTarget(entry.id); } else if (game.library_view === "all") void invoke("collect_and_open_logs", { gameName: game.name, gameDir: game.directory || game.path }).catch(error => notifyError("Logs", error)); else setGameLogsTarget(game); return; }
    if (action === "stop") {
      if (gameInstaller.busy || libraryMutationBusy) return;
      if (game.library_view === "nexus") {
        const entry = nexus.workspace.games.find(item => item.game.path === game.path && item.game.launcher === game.launcher);
        if (entry && (entry.running || nexus.launching === entry.id) && (!nexus.busy || nexus.launching === entry.id)) return nexus.stop(entry.id);
      } else if (gameExecution.pendingPath === game.path || gameExecution.sessions.some(item=>item.path===game.path&&["starting","running"].includes(item.state))) return gameExecution.cancel(game.path);
      return;
    }
    if (gameInstaller.busy || libraryMutationBusy || (gameExecution.active && !(action==="start"&&game.launcher==="PeliGames")) || gameExecution.pendingPath) return;

    if (action === "duplicate" && game.launcher === "PeliGames") {setCopyTarget(game);return;}
    if (action === "start") {
      if (game.library_view === "nexus") {
        const entry = nexus.workspace.games.find(item => item.game.path === game.path && item.game.launcher === game.launcher);
        if (entry) return entry.running ? nexus.stop(entry.id) : nexus.start(entry.id);
        return;
      }
      if (game.launcher === "PeliGames") return gameExecution.start(game.path);
      // Launch errors go to the bell only: the execution state already reports a Steam failure there.
      const execution = t("ui-app", "game_execution");
      if (game.launcher === "Steam") return game.app_id ? gameExecution.startSteam(game.path, game.app_id, game.external_id || undefined).catch(() => {}) : notifyError(execution, t("ui-app", "steam_appid_unavailable"));
      else if (game.external_id) return gameExecution.startExternal(game.path, game.external_id).catch(() => {});
    } else if (game.launcher === "PeliGames") setUninstallTarget({path: game.path, name: game.name, prefix: game.prefix || ""});
    else if (game.launcher === "Steam") void invoke("request_steam_uninstall", { appId: game.app_id }).catch(error => notifyError(t("ui-app", "uninstall"), error));
  };

  const changeSelectedExecutable = () => {
    if (!selectedGame || executableAlternatives.length < 2 || libraryMutationBusy || gameInstaller.busy || gameExecution.active || gameExecution.pendingPath) return;
    libraryDialogs.offerExecutables(executableAlternatives, selectedGame.prefix || "", true);
  };

  // Successful registration is not a user request to abandon the draft. In
  // particular, async installation callbacks can still capture pre-save state.
  const showCompletedGameEntry = (game: GameInfo) => {
    setLibraryGameRemoved({ ...game, library_view: "own" }, false);
    pendingDraftExit.current = null;
    setShowDiscardDraft(false);
    installationDraft.reset();
    installationLocation.reset();
    setInstallationExecutable("");
    setInstallationProton("");
    setInstallationOpen(false);
    setModManagerOpen(false);
    setCoverContextMenu(null);
    setLibraryMutationError("");
    setIsCollapsingGame(false);
    setPlayGameEntrance(true);
    setSelectedGame(game);
    setGamePanelMode("installed");
    setLibraryScope("own");
    setGameDir(game.directory || game.path);
  };

  const submitGameEntry = async () => {
    if (!installationReady || libraryMutationLock.current || gameInstaller.busy) return;
    setLibraryMutationError("");
    if (addingNexusGame) {
      const game: GameInfo = { name: installationDraft.name, path: installationExecutable.trim(), executable: installationExecutable.trim(), directory: installationLocation.directory.trim(), launcher: "Manual", prefix: nexusAdditionPlatform === "proton" ? nexusAdditionPrefix.trim() : undefined, proton: nexusAdditionPlatform === "proton" ? installationProton : undefined, cover_url: installationDraft.coverUrl, automatic_cover_url: installationDraft.coverUrl };
      if (await nexus.add(game, nexusAdditionPlatform)) {
        collapseGameDetailsNow();
        setGamePanelMode("nexus"); setLibraryScope("all"); setIsCollapsingGame(false);
      }
      return;
    }
    try{if(!await removedPrefixNotice.inspect(installationLocation.directory,installationExecutable))return;}
    catch(error){setLibraryMutationError(String(error));return;}
    const request = { name: installationDraft.name, directory: installationLocation.directory, executable: installationExecutable, proton: installationProton, cover_url: installationDraft.coverUrl };
    if (gamePanelMode === "add") {
      libraryMutationLock.current = true; setLibraryMutationBusy(true);
      try {
        const game = await addPeliGamesEntry(request);
        showCompletedGameEntry(game);
        window.dispatchEvent(new Event("peligamesLibraryChanged"));
      } catch (error) { setLibraryMutationError(String(error)); }
      finally { libraryMutationLock.current = false; setLibraryMutationBusy(false); }
    } else {
      const result = await gameInstaller.start(request);
      if (result?.registered_count) {
        const entries = await listPeliGamesEntries().catch(() => []);
        const found = entries.filter(entry => entry.prefix === result.prefix);
        const favourite = found.filter(entry => entry.recommended);
        if (found.length > 1 && favourite.length === 1) {
          // One clear favourite (installer shortcut, engine layout, name): no question.
          const game = await choosePeliGamesEntry(favourite[0].path).catch(() => favourite[0]);
          showCompletedGameEntry(game);
          window.dispatchEvent(new Event("peligamesLibraryChanged"));
        }
        // Several possible executables: the player confirms the game's one.
        else if (found.length > 1) libraryDialogs.offerExecutables(found, result.prefix);
        else if (found[0]) showCompletedGameEntry(found[0]);
      }
    }
  };
  const removeOnlyUninstallTarget=async()=>{
    if(!uninstallTarget||libraryMutationLock.current||gameInstaller.busy||gameExecution.active)return;
    libraryMutationLock.current=true;setLibraryMutationBusy(true);setLibraryMutationError("");
    try{
      await invoke("remove_peligames_library_entry",{path:uninstallTarget.path});
      setUninstallTarget(null);setSelectedGame(null);setModManagerOpen(false);setGameDir("");
      setHomeRevision(value=>value+1);window.dispatchEvent(new Event("peligamesLibraryChanged"));
    }catch(error){setLibraryMutationError(String(error));}
    finally{libraryMutationLock.current=false;setLibraryMutationBusy(false);}
  };
  const confirmPrefixUninstall = async (options: UninstallOptions) => {
    if (!uninstallTarget || libraryMutationLock.current || gameInstaller.busy || gameExecution.active || gameExecution.pendingPath) return;
    libraryMutationLock.current = true; setLibraryMutationBusy(true); setLibraryMutationError("");
    try {
      await uninstallPeliGamesEntry(uninstallTarget.path, options);
      setUninstallTarget(null);
      setSelectedGame(null); setInstallationOpen(false); setModManagerOpen(false); setGameDir("");
      gameInstaller.reset(); installationDraft.reset(); installationLocation.reset(); setInstallationExecutable(""); setInstallationProton("");
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
    } catch (error) { setLibraryMutationError(String(error)); }
    finally { libraryMutationLock.current = false; setLibraryMutationBusy(false); }
  };

  const { onMove: handleCoverMouseMove, onLeave: handleCoverMouseLeave } = useCoverTilt(performanceMode, selectedGame?.path);

  const handleChooseCustomPath = async () => {
    if (!selectedGame || directoryPickerBusy) return;
    const game = selectedGame;
    setDirectoryPickerBusy(true);
    try {
      const path = await openDirectoryPicker(t("gameInfo", "editDirectory"), selectedGameDirectory || game.path);
      if (!path || path === (selectedGameDirectory || game.path)) return;
      const paths=await invoke<{root:string;executable?:string;dlss_destination?:string}>("set_game_mod_destination",{gameKey:game.external_id||game.path,directory:selectedGameRoot||game.directory||game.path,destination:path});
      modPathCache.current.set(modPathKey,paths);setModPaths({...paths,key:modPathKey});
      setGameDir(path);
    } catch (error) {
      notifyError(t("gameInfo", "editDirectory"), error);
    } finally {
      setDirectoryPickerBusy(false);
    }
  };

  const [hasOverlayDialog, setHasOverlayDialog] = useState(false);
  useEffect(() => {
    // Include dialogs owned by child panels and rendered through portals.
    const update = () => setHasOverlayDialog(Array.from(document.querySelectorAll<HTMLElement>(".modal-overlay, .preferences-overlay"))
      .some(overlay => overlay.getClientRects().length > 0));
    const observer = new MutationObserver(update);
    observer.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ["hidden"] });
    update();
    return () => observer.disconnect();
  }, []);
  const hasBlockingDialog = Boolean(droppedPath) || hasOverlayDialog || showDiscardDraft || showGameEntryModal || Boolean(uninstallTarget) || Boolean(gameLogsTarget) || showWelcome || Boolean(appImageTarget) || showModManagerModal || launcherUpdate.open || showSetupWizard || showCreditsModal || showInstructionsModal || showUpdaterModal || modInstallation.hasDialog;
  const hasOpenDialog = (detailsOpen && gameDetailsOverlay && !showSettingsModal) || hasBlockingDialog;
  const fileDropHover = useLauncherFileDrop(hasOpenDialog || preferencesBusy || gameInstaller.busy || libraryMutationBusy || nexus.busy || hasUnfinishedDraft, setDroppedPath);

  useEffect(() => {
    if (hasOpenDialog) {
      setShowSettings(false);
      setCoverContextMenu(null);
    }
  }, [hasOpenDialog]);

  usePreferencesRequests({
    coverBlocked: hasOpenDialog,
    runnerBlocked: hasBlockingDialog || preferencesBusy || gameInstaller.busy || libraryMutationBusy || nexus.busy || nexus.settingsBusy,
    featureBlocked: modInstallation.busy || preferencesBusy || showDiscardDraft,
    open: (section, page, closeMenu) => {
      if (closeMenu) setShowSettings(false);
      setSettingsSection(section); if (page) setSettingsPage(page); setShowSettingsModal(true);
    },
  });

  const viewDownloadedMod = (gameId: string, modId: string) => requestDraftExit(() => { const entry = nexus.workspace.games.find(item => item.id === gameId); if (!entry) return; setGameModsTab("nexus"); setNexusModToView({gameId,modId,requestId: ++modViewRequest.current}); setNexusTab("mods"); setShowWelcome(false); setGamePanelMode("nexus"); setLibraryScope("all"); setModManagerOpen(true); setIsCollapsingGame(false); setSelectedGame({ ...entry.game, nexus_platform: entry.platform, nexus_id: entry.id }); setGameDir(entry.game.directory || entry.game.path); });
  const detailsSummary = (gamePanelMode === "install" || gamePanelMode === "add") ? <InstallGameSummary nexusAddition={addingNexusGame} mode={gamePanelMode} draft={installationDraft} ready={installationReady} busy={gameInstaller.busy || libraryMutationBusy || (addingNexusGame && nexus.busy)} installationResult={gameInstaller.result} installationError={libraryMutationError || (addingNexusGame ? nexus.error : gameInstaller.error)}
              onInstall={() => void submitGameEntry()}
              onUninstall={gameInstaller.result ? () => setUninstallTarget({ path: gameInstaller.result!.prefix.replace(/[\\/]prefix[\\/]?$/, ""), prefix: gameInstaller.result!.prefix, name: installationDraft.name }) : undefined}
              onCollapse={collapseGameDetails} onCoverMove={handleCoverMouseMove} onCoverLeave={handleCoverMouseLeave} reducedMotion={performanceMode} /> : gamePanelMode === "nexus" && !selectedGame ? <NexusEmptySummary onAdd={openNexusAddition} onCollapse={collapseGameDetails} loaded={nexus.loaded} busy={nexus.busy} reducedMotion={performanceMode} onCoverMove={handleCoverMouseMove} onCoverLeave={handleCoverMouseLeave} /> : !selectedGame ? <InstallGameSummary draft={installationDraft} mode="mods" onCollapse={collapseGameDetails} onCoverMove={handleCoverMouseMove} onCoverLeave={handleCoverMouseLeave} reducedMotion={performanceMode} /> : null;
  const selectedCover = selectedGame ? <SelectedGameCover game={selectedGame}
    key={`${gameViewKey(selectedGame)}-${performanceMode ? "performance" : "elegant"}`}
    preparing={gamePanelMode === "nexus"
      ? Boolean(nexusGame && nexus.launching === nexusGame.id && nexus.stopping !== nexusGame.id && !nexusGame.running)
      : Boolean((gameExecution.pendingPath === selectedGame.path || (selectedSession?.path === selectedGame.path && selectedSession.state === "starting")) && !(selectedSession?.path === selectedGame.path && selectedSession.state === "running"))}
    coverRef={coverFlight.destinationRef}
    platform={gamePanelMode === "nexus" ? nexusGame?.platform : undefined}
    onMove={handleCoverMouseMove} onLeave={handleCoverMouseLeave} onContextMenu={setCoverContextMenu} /> : null;
  const nexusPanels = <NexusPanels toolState={nexusToolState} highlightRequestId={nexusModToView?.requestId} highlightedModId={nexusModToView?.gameId === nexusGame?.id ? nexusModToView?.modId : undefined} tab={nexusTab} onTabChange={tab => { if (tab === "mods") setNexusTab(tab); else void requireNexusAccount(language).then(connected => { if (connected) setNexusTab(tab); }); }} game={nexusGame} busy={nexus.busy} settingsBusy={nexus.settingsBusy} progress={nexusGame ? nexus.progress[nexusGame.id] : undefined} error={nexus.error} reducedMotion={performanceMode} onImport={nexus.importArchive} onRemove={nexus.removeArchive} onInstall={nexus.install} onToggle={nexus.toggle} onMove={nexus.move} onSwitchProfile={nexus.switchProfile} onDownloadLoader={nexus.downloadLoader} onLogs={setNexusLogTarget} onConfigure={nexus.configureProton} onConfigureDeploy={nexus.configureDeploy} onConfigureLaunch={nexus.configureLaunch} onUninstall={nexusGame && ["PeliGames", "Steam"].includes(nexusGame.game.launcher) ? () => {void coverAction("uninstall", {...nexusGame.game, prefix:nexusGame.compat_data || nexusGame.game.prefix});} : undefined} onConfigureAdvanced={nexus.configureAdvanced} onError={nexus.setError} />;
  const installedPanel = (section: InstalledPanelSection) => <InstalledGamePanels key={selectedGame?.path} section={section} game={selectedGame} onChangeExecutable={selectedGame?.launcher === "PeliGames" && executableAlternatives.length > 1 ? changeSelectedExecutable : undefined} onUninstall={selectedGame && ["PeliGames", "Steam"].includes(selectedGame.launcher) ? () => {void coverAction("uninstall", selectedGame);} : undefined} disabled={gameInstaller.busy || libraryMutationBusy || gameExecution.active || Boolean(nexusGame?.running) || Boolean(nexus.launching) || nexus.busy || nexus.settingsBusy || Boolean(gameExecution.pendingPath)}
              onSaved={game => { setSelectedGame(previous => previous && previous.path === game.path ? game : previous); if(nexusGame && game.advanced)void nexus.configureAdvanced(nexusGame.id,game.advanced); }} programsDisabled={gameInstaller.busy || libraryMutationBusy || nexus.busy || nexus.settingsBusy || Boolean(nexus.launching) || Boolean(gameExecution.pendingPath)}
              onRunProgram={gameExecution.runProgram} onStartShortcut={gameExecution.startShortcut} />;
  const installationPanels = <InstallGamePanels primaryAction={<InstallGameSubmit mode={gamePanelMode === "add" ? "add" : "install"} ready={installationReady} busy={gameInstaller.busy || libraryMutationBusy || (addingNexusGame && nexus.busy)} onInstall={() => void submitGameEntry()} />} onClose={collapseGameDetails} mode={gamePanelMode === "add" ? "add" : "install"} draft={installationDraft} directory={installationLocation.directory} directorySelected={installationLocation.selected} onDefaultDirectory={installationLocation.selectDefault} directoryError={installationLocation.error} busy={gameInstaller.busy || libraryMutationBusy || (addingNexusGame && nexus.busy)} nexusAddition={addingNexusGame ? { platform: nexusAdditionPlatform, onPlatformChange: setNexusAdditionPlatform, prefix: nexusAdditionPrefix, onPrefixChange: setNexusAdditionPrefix } : undefined} onDirectoryChange={installationLocation.setDirectory}
                executable={installationExecutable} onExecutableChange={setInstallationExecutable} proton={installationProton} onProtonChange={setInstallationProton} />;
  // ---- New interface (Settings → Interface → layout) ----
  const modernSection: ModernSection = showSettingsModal ? "settings" : (gamePanelMode === "install" || gamePanelMode === "add") && installationOpen ? "install" : "library";
  const navigateModern = (section: ModernSection) => {
    if (section === "discord") { window.dispatchEvent(new Event(OPEN_DISCORD_PANEL_EVENT)); return; }
    if (section === "community") { window.dispatchEvent(new Event(OPEN_MATRIX_CHAT_EVENT)); return; }
    if (section === "donate") { void openUrl(SUPPORT_PROJECT_URL).catch(error => notifyError(t("credits", "support"), String(error))); return; }
    if (section === "youtube") { void openUrl(YOUTUBE_MEMBERSHIP_URL).catch(error => notifyError(t("modern", "membership"), String(error))); return; }
    if (showSettingsModal) { if (preferencesBusy) return; setShowSettingsModal(false); if (section === "library") return; }
    if (section === "library") returnToLauncherHome();
    else if (section === "install") changeGamePanelMode("install");
    else if (section === "dlss") openModLibrary();
    else if (section === "nexus") openNexusLibrary();
    else if (section === "downloads") setDownloadsRequest(value => value + 1);
    else { setSettingsSection("general"); setShowSettingsModal(true); }
  };
  const modernSettingsPage: ModernSettingsPage | null = !showSettingsModal ? null
    : settingsPage === "home" ? "interface" : settingsPage === "backendUpdate" ? "mods" : settingsPage;
  const selectedActivity = selectedGame ? activityOf(gameActivity, selectedGame) : undefined;
  const selectedDays = selectedActivity?.last_played ? daysSince(selectedActivity.last_played) : -1;
  const selectedLastPlayed = selectedDays < 0 ? t("modern", "neverPlayed") : selectedDays === 0 ? t("modern", "playedToday") : selectedDays === 1 ? t("modern", "playedYesterday") : t("modern", "playedDaysAgo").replace("{n}", String(selectedDays));
  const selectedPlaytime = formatPlaytime(selectedActivity?.minutes ?? 0);
  const openSelectedFolder = () => invoke("open_folder", { path: selectedGameRoot }).catch((error: unknown) => {
    modInstallation.showError(t("gameInfo", "openFolder"), String(error));
  });
  const selectedProton = gamePanelMode === "nexus" ? nexusGame?.proton : selectedGame?.proton;
  const overviewTiles = selectedGame ? <SelectedGameOverview selectedGame={selectedGame} gamePanelMode={gamePanelMode}
    info={dlssActive ? analysis : overviewAnalysis} nexusGame={nexusGame} selectedGameRoot={selectedGameRoot}
    releaseDate={releaseDate} installStatus={installStatus} installedDll={installedDll} copyReference={copyReference}
    modPathsChecked={modPaths?.key === modPathKey} modPaths={modPaths} directoryPickerBusy={directoryPickerBusy}
    onChooseCustomPath={handleChooseCustomPath} onDuplicate={setCopyTarget}/> : null;
  // Spotlight actions: one big Play button and quick buttons, wired to the
  // same handlers as the cover menu and the classic panel.
  const usesNexusLaunch = Boolean(nexusGame && (nexusGame.mods.some(m=>m.installed && m.enabled) || nexusGame.profile || selectedGame?.nexus_id));
  const selectedView = selectedGame ? { ...selectedGame, library_view: usesNexusLaunch ? "nexus" as const : gamePanelMode === "mods" ? "all" as const : "own" as const } : null;
  const selectedActive = usesNexusLaunch ? Boolean(nexusGame && (nexusGame.running || nexus.launching === nexusGame.id))
    : Boolean(selectedGame && (gameExecution.pendingPath === selectedGame.path || (selectedSession?.path === selectedGame.path && ["starting", "running", "stopping"].includes(selectedSession.state))));
  const selectedLaunchable = !reservedBase && Boolean(selectedGame && (usesNexusLaunch ? nexusGame : selectedGame.launcher === "PeliGames" || selectedGame.launcher === "Steam" || (gamePanelMode === "installed" && selectedGame.external_id)));
  const selectedRunning = usesNexusLaunch ? Boolean(nexusGame?.running) : Boolean(selectedGame && selectedSession?.path === selectedGame.path && selectedSession.state === "running");
  const selectedPreparing = selectedActive && !selectedRunning && (usesNexusLaunch ? nexus.stopping !== nexusGame?.id : selectedSession?.state !== "stopping" && gameExecution.cancellingPath !== selectedGame?.path);
  const selectedRunKey = usesNexusLaunch ? `nexus:${nexusGame?.id}` : `peligames:${selectedSession?.id || selectedGame?.path}`;
  const playBlocked = usesNexusLaunch ? Boolean(nexusGame && (nexus.stopping === nexusGame.id || (nexus.busy && nexus.launching !== nexusGame.id)))
    : (gameExecution.cancellingPath === selectedGame?.path || (selectedSession?.path === selectedGame?.path && selectedSession?.state === "stopping")) || (!selectedActive && Boolean(gameInstaller.busy || libraryMutationBusy || (gameExecution.active&&selectedGame?.launcher!=="PeliGames") || gameExecution.pendingPath));
  const playSelected = () => {
    if (!selectedView) return;
    if (usesNexusLaunch && nexusGame) { if(selectedActive)void nexus.stop(nexusGame.id);else void (async()=>{if(selectedGame?.advanced && !await nexus.configureAdvanced(nexusGame.id,selectedGame.advanced))return;await nexus.start(nexusGame.id)})();return; }
    void coverAction(selectedActive ? "stop" : "start", selectedView);
  };
  const quick = (icon: Parameters<typeof MenuIcon>[0]["name"], label: string, onClick: () => void, options: { pressed?: boolean; disabled?: boolean; danger?: boolean } = {}) =>
    <HoverTooltip key={label} text={label} hoverDelay={300} anchorClassName="modern-quick-anchor"><button type="button" className={`modern-quick-btn${options.danger ? " danger" : ""}`} aria-label={label} aria-pressed={options.pressed} disabled={options.disabled} onClick={onClick}><ModernQuickIcon name={icon} /></button></HoverTooltip>;
  const modernActions = selectedGame && selectedView ? <>
    <SteamPendingActions game={selectedGame} disabled={playBlocked || selectedActive} onSaved={game => setSelectedGame(previous => previous?.path === game.path ? {...previous, ...game} : previous)}>
    <button type="button" className={`btn btn-play game-play-button modern-play${selectedActive ? " running" : ""}${selectedPreparing ? " modern-play-preparing" : ""}`} disabled={!selectedLaunchable || playBlocked} aria-busy={selectedPreparing}
      title={reservedBase ? "Base reservada: execute uma cópia overlay." : selectedLaunchable ? undefined : "Adicione ao PeliGames para iniciar por aqui."} onClick={playSelected}>
      <MenuIcon name={reservedBase ? "folder" : selectedActive ? "stop" : "play"} />{reservedBase ? "Base reservada" : selectedActive ? t("gameExecution", "cancel") : t("modern", "play")}
      {selectedRunning && runningGameTimes[selectedRunKey] !== undefined && <GameRunTimer startedAt={runningGameTimes[selectedRunKey]} />}
    </button>
    </SteamPendingActions>
    <div className="modern-quick">
      {quick("heart", t("modern", "favorite"), () => toggleFavorite(selectedView), { pressed: favorites.has(gameViewKey(selectedView)) })}
      {quick("folder", t("modern", "openFolder"), () => void openSelectedFolder())}
      {quick("logs", t("modern", "logs"), () => { if (gamePanelMode === "nexus") { if (nexusGame) setNexusLogTarget(nexusGame.id); } else coverAction("logs", selectedView); })}
      {nexusGame && quick("bug", t("modern", "reportError"), () => setNexusReportTarget(nexusGame.id))}
      {gamePanelMode === "installed" && selectedGame.launcher === "PeliGames" && quick("trash", t("gameInfo", "uninstall"), () => { void coverAction("uninstall", selectedView); }, {
        danger: true,
        disabled: gameInstaller.busy || libraryMutationBusy || gameExecution.active || Boolean(gameExecution.pendingPath),
      })}
    </div>
  </> : null;
  // DLSS 5 NR tab: install or repair first, then its options.
  const dlssInstallPanel = dlssActive ? <ModernDlssPanel route={route} bitness={bitness} gpuArch={gpuArch}
    recommendedBitness={detectedBitness} recommendOptiscaler={recommendOptiscaler}
    busy={modInstallation.busy || modPaths?.key !== modPathKey} onRoute={value => { if (selectedGame) manualRoutes.current.set(selectedGame.path, value); changeInstallRoute(value); }} onBitness={setBitness}
    recommendation={recommendOptiscaler ? (t("ui-app", "recommended_for_this_game_optiscaler_based_on_the_detected_upscalers")) : undefined}
    startup={neuralStartup.enabled} startupDisabled={modInstallation.busy || neuralStartup.initializing || installStatus === "Verificando..."}
    onStartup={() => neuralStartup.change(!neuralStartup.enabled)} renodx={neuralStartup.renodx} onRenodx={() => neuralStartup.changeRenodx(!neuralStartup.renodx)}
    unifiedNetworks={Boolean(neuralModels?.unified_networks)} experimentalAvailable={Boolean(neuralModels?.experimental)} nrPlugin={Boolean(neuralModels?.nr_plugin)}
    lumeniteAvailable={Boolean(neuralModels?.lumenite)} lumenite={lumeniteChoices[gameDir] ?? neuralModels?.lumenite_selected ?? false}
    onLumenite={value => setLumeniteChoices(previous => ({...previous, [gameDir]: value}))}
    model={neuralModel} installed={isInstalledMod(installStatus)} modelAvailable={Boolean(neuralModels?.small && neuralModels.original && neuralModels.selectable)}
    modelStatus={neuralModelsError ? "error" : !neuralModels ? "checking" : !neuralModels.selectable ? "backend" : !neuralModels.small ? "missing" : "ready"}
    onModel={value => {
      if (value !== neuralModel && isInstalledMod(installStatus)) {
        notifyInfo("DLSS 5 NR", t("ui-app", "model_selected_click_repair_reinstall_to_apply_the_change_to_the_game"));
      }
      setNeuralModel(value); if (value === "small" || value === "experimental") setInt4Mixed(false);
    }}
    int4={int4Mixed} onInt4={() => setInt4Mixed(value => !value)}
    actions={<>
      <button type="button" className="modern-dlss-primary" disabled={modInstallation.busy || modPaths?.key !== modPathKey || neuralStartup.busy || neuralStartup.initializing || installStatus === "Verificando..." || (gpuArch === "rdna3" && bitness === "32")}
        onClick={() => void handleInstall()}>{modInstallation.busy ? t("installAction", "loading") : isInstalledMod(installStatus) ? (t("ui-app", "repair_reinstall")) : t("installAction", "install")}</button>
      {isInstalledMod(installStatus) && <button type="button" className="modern-mini-btn danger" disabled={modInstallation.busy || neuralStartup.busy} onClick={() => modInstallation.requestUninstall(selectedGameDirectory!)}><MenuIcon name="trash" />{t("gameInfo", "uninstall")}</button>}
    </>}
    shortcut={<ShortcutKeySelector key={`${selectedGame?.path || gameDir}:${route}`} disabled={modInstallation.busy || installStatus === "Verificando..."} route={route} shortcutKey={menuShortcutForRoute(preferences.draft.game_shortcut_keys, selectedGame?.path || gameDir, gameDir, route)} setShortcutKey={val => {
      if (appConfig && selectedGame) {
        void changeMenuShortcut({path: selectedGame.path, directory: selectedGameDirectory, route, key: val, installed: isInstalledMod(installStatus)})
          .catch(error => notifyError(t("ui-app", "menu_shortcut"), String(error)));
      }
    }} />}
    feedback={<>{neuralStartup.feedback && <p role="status" className="neural-startup-feedback">{t("neuralStartup", neuralStartup.feedback)}</p>}
      {neuralStartup.error && <p role="alert" className="neural-startup-error">{t("neuralStartup", "error")} {neuralStartup.error}</p>}</>}
  /> : null;
  const installedModCount = Math.max(nexusGame?.mods.filter(mod => mod.installed).length ?? 0, nexusGame?.detected_mods?.filter(mod => mod.enabled).length ?? 0);
  const supportsExecutionSettings = selectedGame?.launcher === "PeliGames" || Boolean(selectedGame?.external_id);
  const executionPanel = (section: "performance" | "options" | "advanced") => supportsExecutionSettings ? installedPanel(section) : nexusGame ? section === "performance" ? <GamePerformancePanel game={{path:String(nexusGame.game.path||""),executable:String(nexusGame.game.executable||""),flatpak:Boolean(nexusGame.game.flatpak)}} value={nexusGame.advanced} disabled={nexus.busy || nexus.settingsBusy || Boolean(nexusGame.running)} onChange={value=>nexus.configureAdvanced(nexusGame.id,value)}/> : cloneElement(nexusPanels,{section}) : <div className="modern-tile"><h3>{t("modern", section === "advanced" ? "options" : section)}</h3><p>{t("ui-app", "this_game_s_launch_options_are_managed_by_its_original_launcher")}</p></div>;
  const dlssGamePanel = selectedGame?.native_linux ? <div className="modern-tile"><h3>DLSSNR-AMD</h3><p>{t("ui-app", "this_is_a_native_linux_installation_the_dlssnr_amd_dll_installer_targets_windows_games_r")}</p></div> : !hasDlssnrConfiguration(appConfig) ? <div className="modern-tile"><h3>DLSSNR-AMD</h3><p>{t("ui-app", "configure_the_backend_and_model_before_installing_dlssnr_amd_in_this_game")}</p><button className="modern-mini-btn" onClick={()=>setShowSetupWizard(true)}>{t("ui-app", "initial_setup")}</button></div> : modPathsError ? <div className="modern-tile" role="alert">{modPathsError}</div> : modPaths?.key !== modPathKey ? <p role="status">{t("ui-app", "locating_the_executable_and_dlssnr_amd_destination")}</p> : !modPaths.dlss_destination ? <div className="modern-tile"><p>{t("ui-app", "could_not_locate_this_installation_s_executable_choose_its_folder_to_configure_dlssnr_am")}</p><button className="modern-mini-btn" disabled={directoryPickerBusy} onClick={()=>void handleChooseCustomPath()}>{t("ui-app", "choose_destination")}</button></div> : <><div className="modern-overview-paths">{<GamePathTile label={t("ui-app", "dlssnr_amd_destination_executable_folder")} path={modPaths.dlss_destination} action={<button className="modern-mini-btn" disabled={directoryPickerBusy || modInstallation.busy} onClick={()=>void handleChooseCustomPath()}>{t("ui-app", "change_destination")}</button>}/>}</div>{dlssInstallPanel}</>;
  const danielTestPanel = experimentalDanielNr && selectedGame && <DanielNrTestPanel
    key={detailsResetKey}
    game={!supportsExecutionSettings && nexusGame ? {...selectedGame, advanced: nexusGame.advanced} : selectedGame}
    disabled={modInstallation.busy || gameInstaller.busy || libraryMutationBusy || nexus.busy || nexus.settingsBusy}
    running={gameExecution.active || Boolean(nexusGame?.running)}
    onBusy={setLibraryMutationBusy}
    onPersist={!supportsExecutionSettings && nexusGame ? async settings => {
      if (!await nexus.configureAdvanced(nexusGame.id, settings)) throw new Error("Não foi possível salvar o perfil de execução.");
    } : undefined}
    onSaved={game => {
      setSelectedGame(previous => previous?.path === game.path ? game : previous);
      if (supportsExecutionSettings && nexusGame) void nexus.configureAdvanced(nexusGame.id, game.advanced!);
    }}
  />;
  const reportGame = selectedGame ? {...selectedGame, nexus_id: nexusGame?.id ?? selectedGame.nexus_id,
    advanced: !supportsExecutionSettings && nexusGame ? nexusGame.advanced : selectedGame.advanced, proton: selectedProton || selectedGame.proton} : null;
  const withFeatureReport = (category: ReportCategory, label: string, content: ReactNode) => reportGame
    ? <FeatureReportSection key={detailsResetKey + ":" + category} context={featureReportContext(reportGame, category, selectedGameRoot || reportGame.directory || reportGame.path, modPaths?.key === modPathKey ? modPaths.dlss_destination || "" : "")} label={label}>{content}</FeatureReportSection> : content;
  const modernTabs: ModernTab[] = gamePanelMode === "install" || gamePanelMode === "add"
    ? [{ id: "installation", label: t("modern", "installation"), content: <div className="modern-install-panels">{installationPanels}</div> }]
    : [
      { id: "overview", label: t("modern", "overview"), content: overviewTiles },
      { id: "nexus", label: t("modern", "mods"), count:installedModCount, content: selectedGame && <GameModsTabs activeTab={gameModsTab} onTabChange={setGameModsTab} nexus={<NexusGameTab key={detailsResetKey} game={selectedGame} workspace={nexus}>{cloneElement(nexusPanels,{section:"mods"})}</NexusGameTab>} dlss={withFeatureReport("dlssnr", "DLSSNR-AMD", dlssGamePanel)} daniel={danielTestPanel} /> },
      ...(nexusToolState.available ? [{ id:"tools",label:t("ui-app", "tools"),content:cloneElement(nexusPanels,{section:"tools"}) }] : []),
      { id: "frame-generation", disabled: !experimentalFrameGeneration, disabledHint: t("ui-app", "under_development_unavailable_in_this_release"), label: t("ui-app", "frame_generation"), content: withFeatureReport("frame-generation", t("ui-app", "frame_generation"), experimentalFrameGeneration && selectedGame && <FrameGeneratorTabs key={detailsResetKey} game={!supportsExecutionSettings && nexusGame ? {...selectedGame,advanced:nexusGame.advanced} : selectedGame} onPersist={!supportsExecutionSettings && nexusGame ? async settings=>{if(!await nexus.configureAdvanced(nexusGame.id,settings))throw new Error("Não foi possível salvar o perfil de execução.");} : undefined} running={gameExecution.active || Boolean(nexusGame?.running)} disabled={Boolean(gameExecution.pendingPath) || Boolean(nexus.launching) || nexus.busy || nexus.settingsBusy} onSaved={game=>{setSelectedGame(game);if(supportsExecutionSettings && nexusGame)void nexus.configureAdvanced(nexusGame.id,game.advanced!);}} />) },
      { id: "performance", label: t("modern", "performance"), content: withFeatureReport("performance", t("ui-app", "performance_and_overlay"), executionPanel("performance")) },
      {id:"captures",label:t("ui-app", "captures"),content: withFeatureReport("captures", t("ui-app", "captures"), selectedGame&&<CapturePanel key={detailsResetKey} gameKey={selectedGame.external_id||selectedGame.path} gameName={selectedGame.name} gameDirectory={selectedGameRoot} gameExecutable={selectedGame.executable||""} steamAppId={selectedGame.launcher==="Steam"?selectedGame.app_id||"":""}/>)},
      { id: "options", label: t("modern", "options"), content: withFeatureReport("options", t("ui-app", "launch_options"), <>{executionPanel("options")}{selectedGame && <PeliRunProfile key={detailsResetKey} game={!supportsExecutionSettings && nexusGame ? {...selectedGame,advanced:nexusGame.advanced} : selectedGame} disabled={modInstallation.busy || gameInstaller.busy || libraryMutationBusy || nexus.busy || nexus.settingsBusy} />}</>) },
      { id: "advanced", label: t("ui-app", "advanced"), content: withFeatureReport("advanced", t("ui-app", "advanced"), executionPanel("advanced")) },
      { id: "reset", label: t("ui-app", "reset"), content: selectedGame&&<GameResetPanel key={detailsResetKey} game={selectedGame} nexusId={nexusGame?.id} gpuArch={gpuArch} bitness={bitness} disabled={modInstallation.busy||gameInstaller.busy||libraryMutationBusy||gameExecution.active||Boolean(gameExecution.pendingPath)||Boolean(nexusGame?.running)||Boolean(nexus.launching)||nexus.busy||nexus.settingsBusy||reservedBase} onBusy={busy=>{setLibraryMutationBusy(busy);if(!busy){nexus.refresh();setHomeRevision(value=>value+1);setGameInfoRevision(value=>value+1);}}} onSaved={game=>{setSelectedGame(previous=>previous?.path===game.path?game:previous);}}/> },
    ];
  if(reservedBase){for(const tab of modernTabs){if(tab.id!=="overview"){tab.disabled=true;tab.disabledHint="Base reservada para cópias overlay. Use as configurações de uma cópia.";tab.content=<p>Base reservada. Configure uma das cópias overlay.</p>;}}}
  const gameGrid = <GameGrid key="unified-library" supplementalGames={(nexus.loaded ? nexus.workspace.games : cachedNexus.games).map(item=>({...item.game,nexus_id:item.id,nexus_platform:item.platform}))} onCoverAction={coverAction} coverHoldBlocked={gameInstaller.busy || libraryMutationBusy || nexus.busy || Boolean(gameExecution.pendingPath)} coverActionsBlocked={gameInstaller.busy || libraryMutationBusy || nexus.busy || Boolean(gameExecution.pendingPath)} homeRevision={homeRevision} onSelectGame={handleSelectGame} startingGamePath={gameExecution.pendingPath || (gameExecution.session?.state === "starting" ? gameExecution.session.path : undefined)} runningGamePaths={[...gameExecution.sessions.filter(item=>item.state==="running"||item.state==="stopping").map(item=>item.path),...nexus.workspace.games.filter(item=>item.running).map(item=>item.game.path)]} startingGamePaths={[...gameExecution.sessions.filter(item=>item.state==="starting").map(item=>item.path),...nexus.workspace.games.filter(item=>item.id===nexus.launching&&!item.running).map(item=>item.game.path)]} selectedGameKey={selectedGame ? gameViewKey(selectedGame) : undefined} mode={gamePanelMode === "mods" || gamePanelMode === "nexus" ? "installed" : gamePanelMode} libraryScope="own" modeDisabled={gameInstaller.busy || libraryMutationBusy || nexus.busy} onModeChange={changeGamePanelMode} onCountChange={setLibraryCount} />;

  const selectedBanner = useGameBanner(selectedGame || undefined, !["install", "add"].includes(gamePanelMode));
  const installingGame = gamePanelMode === "install" || gamePanelMode === "add";
  const installationBanner = useGameBanner(installingGame && installationDraft.name && !installationDraft.editing ? { name: installationDraft.name, path: "installation-draft", launcher: "PeliGames", cover_url: installationDraft.coverStatus === "found" ? installationDraft.coverUrl : undefined } : undefined, true);
  const modernCatalogOpen = unifiedTab === "nexus" && gameModsTab === "nexus" && detailsOpen && Boolean(nexusGame) && nexusTab === "catalog";
  const modernDetails = <GameBenchmarkNavigationContext.Provider value={selectedGame ? openGameBenchmarks : null}><ModernGameDetails resetKey={detailsResetKey}
    contentView={benchmarkOpen && selectedGame ? <GameBenchmarkView key={detailsResetKey} game={selectedGame} onBack={() => setBenchmarkViewKey(null)} /> : undefined}
    activeTab={installingGame ? undefined : detailsReturnTab?.key === detailsResetKey ? detailsReturnTab.tab : unifiedTab}
    onTabChange={tab => {if(tab !== "nexus") setNexusModToView(null);setUnifiedTab(tab);setDetailsReturnTab({ key: detailsResetKey, tab });}}
    artSharp={Boolean(installingGame ? installationBanner : selectedBanner)}
    artwork={installingGame ? [installationBanner, installationDraft.coverUrl].filter((url): url is string => Boolean(url)) : selectedGame ? [
      ...(selectedBanner ? [selectedBanner] : []),
      ...(selectedGame.cover_url ? [selectedGame.cover_url] : []),
    ] : undefined}
    summary={detailsSummary ?? undefined} cover={selectedCover}
    eyebrow={selectedGame ? <><LauncherIcon launcher={selectedGame.launcher} size={14} />{selectedGame.launcher}</> : undefined}
    title={selectedGame?.name} meta={selectedGame ? <>
      {selectedPlaytime && <span className="modern-meta-chip">{t("modern", "hoursPlayed").replace("{n}", selectedPlaytime)}</span>}
      {selectedActivity?.last_played ? <span className="modern-meta-chip">{selectedLastPlayed}</span> : null}
      {gamePanelMode !== "nexus" && dlssNrInstalled && <span className="modern-meta-chip ok">{t("modern", "dlssInstalled")}</span>}
      {installedModCount > 0 && <span className="modern-meta-chip">{installedModCount} {installedModCount === 1 ? "mod" : "mods"}</span>}
      <GameConfigurationChips gameKey={selectedGame.external_id||selectedGame.path} advanced={supportsExecutionSettings?selectedGame.advanced:nexusGame?.advanced??selectedGame.advanced}/>
    </> : undefined}
    actions={modernActions ?? undefined}
    tabRequest={nexusModToView && nexusModToView.gameId === nexusGame?.id ? {tab:"nexus",revision:nexusModToView.requestId} : undefined}
    catalog={modernCatalogOpen ? <NexusCatalogPanel key={nexusGame!.id} embedded game={nexusGame} onAssociate={nexus.configureDomain} onImport={nexus.importArchive}
      onClose={() => setNexusTab("mods")} onLibrary={() => { setNexusTab("mods"); collapseGameDetails(); }} /> : undefined}
    tabs={modernTabs} onClose={() => {setNexusModToView(null);collapseGameDetails();}} closeLabel={t("modern", "closeDetails")} /></GameBenchmarkNavigationContext.Provider>;
  const modernMain = <ModernShell active={modernSection} disabled={hasOpenDialog || (showSettingsModal && preferencesBusy)} onNavigate={navigateModern} activeDownloads={activeDownloads}
    libraryCount={libraryCount} settingsPage={modernSettingsPage}
    onSettingsNavigate={next => { if (!preferencesBusy) setSettingsPage(next); }}>
    {showSettingsModal && <div className="modern-scroll modern-settings-scroll">
      <SettingsModal page={settingsPage} onPageChange={setSettingsPage} onBusyChange={setPreferencesBusy} initialSection={settingsSection} initialBackendArch={settingsSection === "backendUpdate" ? notificationBackendArch : undefined}
        onClose={() => setShowSettingsModal(false)}
        onConfigUpdated={config => { if (!config) setShowSetupWizard(true); }}
        onOpenWizard={() => setShowSetupWizard(true)}
        onOpenWelcome={() => setShowWelcome(true)} />
    </div>}
    <div ref={launcherScroll} className="container modern-scroll" style={gameDetailsOverlay && detailsOpen ? { overflow: "hidden" } : undefined} hidden={showSettingsModal}>
      <AnimatePresence onExitComplete={() => { if (isCollapsingGame && !coverFlight.returning) { setSelectedGame(null); setIsCollapsingGame(false); } }}>
        {detailsOpen && !gameDetailsOverlay && <motion.section key="game-details" className="selected-game-shell modern-details-shell"
          initial={performanceMode || !playGameEntrance ? false : { height: 0, opacity: 0, marginBottom: 0 }} animate={{ height: detailsSize.height ?? "auto", opacity: 1, marginBottom: 18 }} exit={{ height: 0, opacity: 0, marginBottom: 0, overflow: "hidden", transition: { duration: performanceMode ? 0 : 0.45, ease: [0.4, 0, 0.2, 1] } }}
          style={{ overflow: detailsSize.resizing && !performanceMode ? "hidden" : "visible" }}
          onAnimationStart={detailsSize.startResize} onAnimationComplete={detailsSize.finishResize}
          transition={{ duration: performanceMode ? 0 : 0.45, ease: [0.2, 0.8, 0.2, 1] }}>
          <div ref={detailsSize.contentRef} className="modern-details-frame">{modernDetails}</div>
        </motion.section>}
      </AnimatePresence>
      <div className="game-library-section modern-library">{gameGrid}</div>
    </div>
  </ModernShell>;

  return (
    <EffectsContext.Provider value={performanceMode}>
      <RuntimeActivityNotice />
      {coverFlight.overlay}
      <AnimatePresence onExitComplete={() => { if (isCollapsingGame && !coverFlight.returning) { setSelectedGame(null); setIsCollapsingGame(false); } }}>
        {detailsOpen && gameDetailsOverlay && !showSettingsModal && <GameDetailsOverlay key="selected-game-overlay" label={selectedGame?.name || "Game details"} closeLabel={t("modern", "closeDetails")} onClose={collapseGameDetails} reducedMotion={performanceMode} flying={coverFlight.flying} catalog={modernCatalogOpen}>
          {modernDetails}
        </GameDetailsOverlay>}
      </AnimatePresence>
      <div className={`app-wrapper${coverFlight.flying ? " cover-flight-active" : ""} modern-ui${showSettingsModal ? " modern-settings-active" : ""}`} style={{ animation: effectsHaveChanged ? "none" : undefined }}>
        <TitleBar
          runningGames={titlebarRunningGames}
          onHome={returnToLauncherHome} homeDisabled={gameInstaller.busy || libraryMutationBusy}
          onShowCredits={() => setShowCreditsModal(true)}
          disabled={hasOpenDialog}
          notificationIndicator={<NotificationCenter onNexusAccount={() => { setShowSettings(false); setSettingsSection("nexusAccount"); setSettingsPage("connections"); setShowSettingsModal(true); }} nexusError={nexus.error} navigationBusy={preferencesBusy || Boolean(launcherUpdate.downloadProgress)} launcherUpdate={launcherUpdate.info} onNavigate={target => {
            setShowSettings(false);
            if (target === "launcher") launcherUpdate.setOpen(true);
            else if (target === "rdna4" || target === "rdna3") { setNotificationBackendArch(target); setSettingsSection("backendUpdate"); setShowSettingsModal(true); }
            else { setSettingsSection("protons"); setShowSettingsModal(true); }
          }}/>}
          updateIndicator={<LauncherUpdateIndicator status={launcherUpdate.status} disabled={hasOpenDialog} downloadProgress={launcherUpdate.downloadProgress}
            onCheck={launcherUpdate.check}
            onOpen={() => { setShowSettings(false); launcherUpdate.setOpen(true); }} />}
          settingsMenu={
            <LauncherSettingsMenu open={showSettings} onOpenChange={setShowSettings} disabled={hasOpenDialog}
              preferencesOpen={showSettingsModal} performanceMode={performanceMode} onPerformanceModeChange={setPerformanceMode}
              onPreferences={() => { setSettingsSection("general"); setShowSettingsModal(true); }} />
          }
        />
        {modernMain}
      <WindowResizeHandles />
      <div className="launcher-build-label">{APP_BUILD_LABEL}</div>
    </div>

    <DlssInstallationDialogs installation={modInstallation} onUninstall={path => void proceedUninstallForPath(path)} onSetup={() => setShowSetupWizard(true)} />

    {launcherUpdate.open && <AppUpdateModal onDownloadProgress={launcherUpdate.setDownloadProgress} initialInfo={launcherUpdate.info ?? undefined} onClose={() => launcherUpdate.setOpen(false)} />}

    {showUpdaterModal && (
      <BackendUpdaterModal
        gpuArch={gpuArch}
        onClose={() => setShowUpdaterModal(false)}
      />
    )}

    {showInstructionsModal && (
      <InstructionsModal onClose={() => setShowInstructionsModal(false)} />
    )}

    {showCreditsModal && (
      <CreditsModal onClose={() => setShowCreditsModal(false)} />
    )}

    {fileDropHover && <div className="launcher-file-drop-hint"><strong>Solte o arquivo ou a pasta aqui</strong><span>Instalar, adicionar à biblioteca ou importar um mod</span></div>}
    {droppedPath && <DropImportDialog path={droppedPath} games={(nexus.workspace.games.length ? nexus.workspace.games : cachedNexus.games).map(entry=>({id:entry.id,name:entry.game.name}))}
      onLoadModGames={async()=>{setCachedNexus(await invoke<import("./modules/nexus/useNexusWorkspace").NexusWorkspace>("load_nexus_workspace"));}}
      onClose={()=>setDroppedPath(null)} onImportMod={async (id, path)=>{
        const imported = await nexus.importArchive(id,path);
        if (!imported) { notifyError("Mod local", "Não foi possível importar o pacote. Confira o arquivo e tente novamente."); return false; }
        const entry=nexus.workspace.games.find(game=>game.id===id) ?? cachedNexus.games.find(game=>game.id===id);
        if(entry){setShowSettingsModal(false);setGamePanelMode("nexus");setLibraryScope("all");setNexusTab("mods");setSelectedGame({...entry.game,nexus_id:entry.id,nexus_platform:entry.platform});setGameDir(entry.game.directory||entry.game.path);setModManagerOpen(true);}
        return true;
      }}/>}
    {showGameEntryModal && <GameEntryModal onClose={() => setShowGameEntryModal(false)} onSelect={mode => requestDraftExit(() => {
      setShowGameEntryModal(false);
      collapseGameDetailsNow(); setIsCollapsingGame(false); setPlayGameEntrance(true);
      setGamePanelMode(mode); setLibraryScope("own"); setInstallationOpen(true); setLibraryMutationError("");
    })} />}
    {showDiscardDraft && <ConfirmModal title={t("gameModes", "unfinishedTitle")}
      message={t("gameModes", "unfinishedMessage")}
      confirmText={t("gameModes", "discardDraft")} cancelText={t("gameModes", "keepEditing")}
      onCancel={() => { pendingDraftExit.current = null; setShowDiscardDraft(false); }}
      onConfirm={() => {
        const action = pendingDraftExit.current;
        pendingDraftExit.current = null;
        setShowDiscardDraft(false);
        collapseGameDetailsNow();
        action?.();
      }} />}
    <LibraryEntryDialogs controller={libraryDialogs} currentPath={selectedGame?.path}/>
    {uninstallTarget && <PrefixUninstallModal name={uninstallTarget.name} prefix={uninstallTarget.prefix} busy={libraryMutationBusy} error={libraryMutationError}
      onCancel={() => { if (!libraryMutationBusy) { setUninstallTarget(null); setLibraryMutationError(""); } }} onConfirm={options => void confirmPrefixUninstall(options)} onRemoveOnly={()=>void removeOnlyUninstallTarget()} />}

    {appImageTarget && !showWelcome && !launcherUpdate.open && <AppImageInstallPrompt target={appImageTarget} onClose={() => setAppImageTarget("")} />}
    {showWelcome && <WelcomeModal onStart={() => {
      localStorage.setItem("peligames_welcome_completed", "1");
      setShowWelcome(false);
      collapseGameDetails();
      setGamePanelMode("install");
      setLibraryScope("own");
    }} onNavigate={destination => {
      localStorage.setItem("peligames_welcome_completed", "1");
      setShowWelcome(false);
      if (destination === "nexus") { openNexusLibrary(); return; }
      if (destination === "dlssnr") { openModLibrary(); return; }
      if (destination === "matrix") { window.dispatchEvent(new Event(OPEN_MATRIX_CHAT_EVENT)); return; }
      if (destination === "install") { changeGamePanelMode("install"); return; }
      if (destination === "library") { returnToLauncherHome(); return; }
      setSettingsPage(destination === "appimageInstall" ? "tools" : destination);
      setSettingsSection(destination); setShowSettingsModal(true);
    }} />}

    {showSetupWizard && (
      <SetupWizard
        allowCancel
        onCancel={closeDlssSetup}
        onComplete={(config) => {
          setShowSetupWizard(false);
          openModLibrary(config);
        }}
      />
    )}

    {showModManagerModal && <ModManagerModal disabled={!appConfigLoaded} onClose={() => setShowModManagerModal(false)} onSelect={integration => {
      setShowModManagerModal(false);
      if (integration === "nexus") { openNexusLibrary(); return; }
      if (hasDlssnrConfiguration(appConfig)) openModLibrary();
      else setShowSetupWizard(true);
    }} />}

    {nexusReportTarget && <NexusReportModal gameId={nexusReportTarget} onClose={() => setNexusReportTarget(null)} />}
    {nexusLogTarget && <NexusLogsModal gameId={nexusLogTarget} onClose={() => setNexusLogTarget(null)} />}

    {gameLogsTarget && <GameLogsModal game={gameLogsTarget} onClose={() => setGameLogsTarget(null)} />}
    {coverContextMenu && selectedGame && <GameCoverMenu ref={coverContextMenuRef}
      game={{ ...selectedGame, library_view: gamePanelMode === "nexus" ? "nexus" : libraryScope }}
      x={coverContextMenu.x} y={coverContextMenu.y} onClose={() => setCoverContextMenu(null)}
      blocked={gameInstaller.busy || libraryMutationBusy || nexus.busy || Boolean(gameExecution.pendingPath)} onAction={coverAction} />}
    {copyTarget && <GameCopyDialog game={copyTarget} onBusy={setLibraryMutationBusy} onClose={()=>setCopyTarget(null)}/>}
    {removeLibraryTarget && <NexusPopup title="Remover da biblioteca" onClose={() => setRemoveLibraryTarget(null)} footer={<>
      <button className="btn btn-secondary" onClick={() => setRemoveLibraryTarget(null)}>Cancelar</button>
      <button className="btn btn-primary" disabled={nexus.busy || libraryMutationBusy} onClick={() => {
        const target = removeLibraryTarget;
        // Managed libraries persist removal; manually scanned folders only hide the card.
        if (target.library_view === "nexus" && target.nexus_id) {
          void nexus.removeGame(target.nexus_id).then(removed => {
            if (!removed) return;
            if (selectedGame?.nexus_id === target.nexus_id) collapseGameDetailsNow();
            setRemoveLibraryTarget(null);
          });
          return;
        }
        if (target.launcher === "PeliGames") {
          setLibraryMutationBusy(true);
          void invoke("remove_peligames_library_entry", { path: target.path }).then(() => {
            setLibraryGameRemoved(target, true);
            if (selectedGame?.path === target.path) collapseGameDetailsNow();
            setRemoveLibraryTarget(null);
            setHomeRevision(value => value + 1);
          }).catch(error => notifyError("Remover da biblioteca", error)).finally(() => setLibraryMutationBusy(false));
          return;
        }
        setLibraryGameRemoved(target, true);
        if (selectedGame?.path === target.path) collapseGameDetailsNow();
        setRemoveLibraryTarget(null);
      }}><MenuIcon name="trash" />Remover</button>
    </>}>{removeLibraryTarget.library_view === "nexus"
      ? <><p>Remover {removeLibraryTarget.name} da biblioteca Nexus? O jogo e os pacotes de mods no disco serão preservados. Um jogo com mods instalados precisa ter os mods removidos antes.</p>{nexus.error && <p role="alert" className="nexus-error">{nexus.error}</p>}</>
      : <p>Remover {removeLibraryTarget.name} desta biblioteca? O jogo e os mods no disco serão preservados, assim como as entradas nas outras bibliotecas.</p>}</NexusPopup>}
    {removedPrefixNotice.dialog}
    <NexusDownloadsPanel games={nexus.workspace.games} hideButton openRequest={downloadsRequest} accountSettingsOpen={showSettingsModal && settingsSection === "nexusAccount"} onActiveCount={setActiveDownloads} onViewMod={viewDownloadedMod} />
    <NexusBrowserPopup />
    </EffectsContext.Provider>
  );
}

export default App;

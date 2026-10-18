import { DropImportDialog } from "./components/DropImportDialog";
import { useLauncherFileDrop } from "./hooks/useLauncherFileDrop";
import { GameBenchmarkView } from "./components/preferences/GameBenchmarkView";
import { GameBenchmarkNavigationContext } from "./components/preferences/GameBenchmarkNavigationContext";
import { requireNexusAccount } from "./modules/nexus/requireNexusAccount";
import { SteamPendingActions } from "./components/SteamPendingActions";
import { useGameBanner } from "./hooks/useGameBanner";
import { openUrl } from "@tauri-apps/plugin-opener";
import { OPEN_DISCORD_PANEL_EVENT, SUPPORT_PROJECT_URL } from "./services/communityLinks";
import { WindowResizeHandles } from "./components/WindowResizeHandles";
import { GamePlatformBadge } from "./components/GamePlatformBadge";
import { listen } from "@tauri-apps/api/event";
import { NexusBrowserPopup } from "./modules/nexus/NexusBrowserPopup";
import { MAX_INTERFACE_SCALE, useInterfaceScale } from "./hooks/useInterfaceScale";
import { type CoverGameAction } from "./components/CoverGameActions";
import { GameLogsModal } from "./components/GameLogsModal";
import { gameViewKey } from "./services/gameIdentity";
import { GameEntryModal } from "./components/GameEntryModal";
import { type UninstallOptions, addPeliGamesEntry, uninstallPeliGamesEntry, listPeliGamesEntries, choosePeliGamesEntry, listPeliGamesExecutableChoices, newLauncherGames, addLauncherGames, type LauncherGame } from "./services/installedLibrary";
import { LauncherGamesModal } from "./components/LauncherGamesModal";
import { notifyError, notifyInfo } from "./services/notifications";
import { ExecutableChoiceModal } from "./components/ExecutableChoiceModal";
import { useGameExecution } from "./hooks/useGameExecution";
import { useTitlebarRunningGames } from "./hooks/useTitlebarRunningGames";
import { HoverTooltip } from "./components/HoverTooltip";
import { ModernQuickIcon } from "./components/modern/ModernQuickIcon";
import { GamePerformancePanel } from "./components/GamePerformancePanel";
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
import { isMissingModel, localizeInstallationMessage } from "./services/installationMessages";
import { runInstallation } from "./services/installerService";
import { openDirectoryPicker } from "./services/tauriService";
import { OPEN_MATRIX_CHAT_EVENT } from "./services/matrixChat";

// Components
import { ResultModal } from "./components/ResultModal";
import { LauncherUpdateIndicator } from "./components/LauncherUpdateIndicator";
import { checkLauncherUpdate, checkLauncherUpdateOnStartup, type LauncherDownloadProgress, type LauncherUpdateInfo, type LauncherUpdateStatus } from "./services/launcherUpdates";
import { AppUpdateModal } from "./components/AppUpdateModal";
import { BackendUpdaterModal } from "./components/BackendUpdaterModal";
import { LoadingModal } from "./components/LoadingModal";
import { CreditsModal } from "./components/CreditsModal";
import { NotificationCenter } from "./components/NotificationCenter";
import { RuntimeActivityNotice } from "./components/RuntimeActivityNotice";
import { TitleBar } from "./components/TitleBar";
import { SetupWizard, AppConfig } from "./components/SetupWizard";
import { SettingsModal, type SettingsPage } from "./components/SettingsModal";
import { useFavorites, toggleFavorite } from "./services/favorites";
import { NexusReportModal } from "./modules/nexus/NexusReportModal";
import { NexusLogsModal } from "./modules/nexus/NexusLogsModal";
import { NexusPopup } from "./modules/nexus/NexusPopup";
import { NexusDownloadsPanel } from "./modules/nexus/NexusDownloadsPanel";
import { useNexusGameTools } from "./modules/nexus/useNexusGameTools";
import { NexusPanels } from "./modules/nexus/NexusPanels";
import { NexusCatalogPanel } from "./modules/nexus/NexusCatalogPanel";
import { NexusEmptySummary } from "./modules/nexus/NexusEmptySummary";
import { useNexusWorkspace } from "./modules/nexus/useNexusWorkspace";
import { useNexusText } from "./modules/nexus/text";
import "./modules/nexus/nexus.css";
import "./theme/controlStability.css";
import { ModManagerModal } from "./components/ModManagerModal";
import { WelcomeModal } from "./components/WelcomeModal";
import { AppImageInstallPrompt, APPIMAGE_PROMPT_KEY } from "./components/AppImageInstallPrompt";
import { bootSplashGone } from "./components/BootSplash";
import { hasDlssnrConfiguration } from "./services/modConfiguration";
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
import { defaultShortcutForRoute, steamLaunchOptionsForRoute, launchOptionsFromInstallOutput } from "./services/routeDefaults";
import { syncModLaunchOptions } from "./services/modLaunchOptions";
// Launch report shown over the library (same order as when Pelinstall was bundled here).
import "./modules/pelinstall/pelinstall.css";

function App({ initialGamePath }: { initialGamePath?: string }) {
  const emptyPreview = import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window ? new URLSearchParams(location.search).get("empty") || new URLSearchParams(location.search).get("demo") : null;
  const [performanceMode, setPerformanceMode] = useState(() => localStorage.getItem("performance_mode") === "true");
  const coverFlight = useCoverFlight(performanceMode);
  // New interface: playtime shown in the game spotlight.
  const [gameActivity, setGameActivity] = useState<ActivityMap>({});
  // New interface: Settings is a page of the window, picked from the side menu.
  const [settingsPage, setSettingsPage] = useState<SettingsPage>("home");
  const [libraryCount, setLibraryCount] = useState(0);
  const favorites = useFavorites();
  const [downloadsRequest, setDownloadsRequest] = useState(0), [activeDownloads, setActiveDownloads] = useState(0);
  useEffect(() => {
    let active = true, revision = 0;
    let unlisten: (() => void) | undefined;
    const showEntry = async (path: string) => {
      const request = ++revision;
      const entries = await listPeliGamesEntries();
      const entry = entries.find(game => game.path === path);
      if (active && request === revision && entry) {
        setShowWelcome(false);
        setSelectedGame(entry); setGamePanelMode("installed"); setLibraryScope("own"); setGameDir(entry.directory || entry.path);
      }
    };
    if (initialGamePath) void showEntry(initialGamePath).catch(console.error);
    if (!("__PELI_UI_PREVIEW__" in window)) {
      const receive = async () => {
        const path = await invoke<string | null>("take_launcher_open_request");
        if (active && path) await showEntry(path);
      };
      void listen("launcher-open-request", () => { void receive().catch(console.error); }).then(stop => {
        if (!active) { stop(); return; }
        unlisten = stop;
        void receive().catch(console.error);
      }).catch(console.error);
    }
    return () => { active = false; unlisten?.(); };
  }, [initialGamePath]);
  const [playGameEntrance, setPlayGameEntrance] = useState(true);
  const [effectsHaveChanged, setEffectsHaveChanged] = useState(false);
  const effectsInitialized = useRef(false);
  useLayoutEffect(() => {
    if (effectsInitialized.current) setEffectsHaveChanged(true);
    effectsInitialized.current = true;
    setPlayGameEntrance(false);
    localStorage.setItem("performance_mode", String(performanceMode));
    document.body.classList.toggle("performance-mode", performanceMode);
  }, [performanceMode]);
  const [gameDir, setGameDir] = useState("");
  const [route, setRoute] = useState<InstallRoute>("optiscaler");
  // Optional parts of the next install or repair, on every route.
  const [neuralModel, setNeuralModel] = useState<"small" | "original">("small");
  const [neuralModelsError, setNeuralModelsError] = useState(false);
  const [neuralModels, setNeuralModels] = useState<{small:boolean; original:boolean; selectable:boolean} | null>(null);
  const [renodxStyle, setRenodxStyle] = useState(false), [int4Mixed, setInt4Mixed] = useState(false);
  const [bitness, setBitness] = useState<"32" | "64">("64");
  const [gpuArch, setGpuArch] = useState<"rdna4" | "rdna3">("rdna4");
  const { t, language, setLanguage } = useI18n();
  const [executableChoice, setExecutableChoice] = useState<{ entries: GameInfo[]; prefix: string; changing?: boolean } | null>(null);
  const [executableChoiceError, setExecutableChoiceError] = useState("");
  const [launcherGames, setLauncherGames] = useState<{ path: string; games: LauncherGame[] } | null>(null);
  const [launcherGamesError, setLauncherGamesError] = useState("");
  const localizeAnalysisValue = (value: string) => {
    if (value === "Linux Nativo" || value === "Native Linux") return t("gameInfo", "nativeLinux");
    if (["Não detectada", "Não detectado", "Not detected"].includes(value)) return t("gameInfo", "notDetected");
    if (["Verificando...", "Checking..."].includes(value)) return t("gameInfo", "verifying");
    if (["Desconhecido", "Desconhecida", "Unknown"].includes(value)) return t("gameInfo", "unknown");
    return value;
  };

  const [loading, setLoading] = useState(false);
  const [loadingMessage, setLoadingMessage] = useState("");
  const [logs, setLogs] = useState("");

  const [showModal, setShowModal] = useState(false);
  const [modelRecovery, setModelRecovery] = useState(false);
  const [modalTitle, setModalTitle] = useState("");
  const [modalMessage, setModalMessage] = useState("");
  const [modalType, setModalType] = useState<"success" | "error">("success");

  const [showAppUpdate, setShowAppUpdate] = useState(false);
  const [launcherDownloadProgress, setLauncherDownloadProgress] = useState<LauncherDownloadProgress | null>(null);
  const [launcherUpdateInfo, setLauncherUpdateInfo] = useState<LauncherUpdateInfo | null>(null);
  const [launcherUpdateStatus, setLauncherUpdateStatus] = useState<LauncherUpdateStatus>("checking");
  const manualUpdateCheck = useRef(false);
  const checkUpdatesManually = async () => {
    if (manualUpdateCheck.current || launcherUpdateStatus === "checking") return;
    manualUpdateCheck.current = true;
    setLauncherUpdateStatus("checking");
    try {
      const info = await checkLauncherUpdate();
      setLauncherUpdateInfo(info);
      setLauncherUpdateStatus(info.available ? "available" : "current");
    } catch {
      setLauncherUpdateStatus("error");
    } finally {
      manualUpdateCheck.current = false;
    }
  };
  // The update channel changed in Settings → Tools: offer what it now allows.
  const recheckUpdates = useRef(checkUpdatesManually); recheckUpdates.current = checkUpdatesManually;
  useEffect(() => {
    const recheck = () => void recheckUpdates.current();
    window.addEventListener("peligamesUpdateChannelChanged", recheck);
    return () => window.removeEventListener("peligamesUpdateChannelChanged", recheck);
  }, []);
  useEffect(() => {
    let active = true;
    checkLauncherUpdateOnStartup().then(info => {
      if (!active) return;
      setLauncherUpdateInfo(info);
      setLauncherUpdateStatus(info.available ? "available" : "current");
    }).catch(() => { if (active) setLauncherUpdateStatus("error"); });
    return () => { active = false; };
  }, []);
  const [showUpdaterModal, setShowUpdaterModal] = useState(false);
  const [showInstructionsModal, setShowInstructionsModal] = useState(false);
  const [showCreditsModal, setShowCreditsModal] = useState(false);
  const [showSetupWizard, setShowSetupWizard] = useState(false);
  const [showWelcome, setShowWelcome] = useState(() => !initialGamePath && localStorage.getItem("peligames_welcome_completed") !== "1");
  // A loose AppImage asks once per start whether to install, unless dismissed for good.
  const [appImageTarget, setAppImageTarget] = useState("");
  useEffect(() => {
    const promptPreview = import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window && new URLSearchParams(location.search).get("appimagePrompt") === "1";
    if (("__PELI_UI_PREVIEW__" in window && !promptPreview) || initialGamePath) return;
    if (!promptPreview) {
      try { if (localStorage.getItem(APPIMAGE_PROMPT_KEY) === "1") return; } catch { /* Ask anyway. */ }
    }
    let active = true;
    void invoke<{ appimage: boolean; installed: boolean; target: string }>("appimage_install_status")
      // Asked over the drawn library, after the start screen has gone.
      .then(async status => { if (!active || !status.appimage || status.installed) return; await bootSplashGone(); if (active) setAppImageTarget(status.target); }).catch(() => {});
    return () => { active = false; };
  }, []);
  const [settingsSection, setSettingsSection] = useState<"general" | "covers" | "protons" | "backendUpdate" | "monitoring" | "diskAutomount" | "interface" | "connections" | "nexusAccount" | "appimageInstall">("general");
  const [preferencesBusy, setPreferencesBusy] = useState(false);
  const [notificationBackendArch, setNotificationBackendArch] = useState<"rdna3" | "rdna4">("rdna4");
  const [showSettingsModal, setShowSettingsModal] = useState(false);
  const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
  const [appConfigLoaded, setAppConfigLoaded] = useState(false);

  const changeInstallRoute = (nextRoute: InstallRoute) => {
    if (nextRoute === route) return;
    setRoute(nextRoute);
  };

  const [showSettings, setShowSettings] = useState(false);
  const [showLangDropdown, setShowLangDropdown] = useState(false);
  const [showEffectsDropdown, setShowEffectsDropdown] = useState(false);
  useEffect(() => {
    if (!showSettings) {
      setShowEffectsDropdown(false);
      setShowLangDropdown(false);
    }
  }, [showSettings]);
  const [directoryPickerBusy, setDirectoryPickerBusy] = useState(false);
  const coverContextMenuRef = useRef<HTMLDivElement>(null);
  const [removeLibraryTarget, setRemoveLibraryTarget] = useState<GameInfo | null>(null);
  const [gameLogsTarget, setGameLogsTarget] = useState<GameInfo | null>(null);
  const [coverContextMenu, setCoverContextMenu] = useState<{ x: number, y: number } | null>(null);
  const settingsRef = useRef<HTMLDivElement>(null);
  const [uiScale, setUiScale] = useInterfaceScale();

  useEffect(() => {
    invoke<AppConfig | null>("load_app_config")
      .then((res) => {
        if (res) {
          setAppConfig(res);
        }
      })
      .catch((err) => {
        console.error("Failed to load app config", err);
      }).finally(() => setAppConfigLoaded(true));
  }, []);

  useEffect(() => {
    if (appConfig) {
      setGpuArch(appConfig.backend === "AMDNR" ? "rdna4" : "rdna3");
    }
  }, [appConfig]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (settingsRef.current && !settingsRef.current.contains(event.target as Node)) {
        setShowSettings(false);
      }
      if (!coverContextMenuRef.current?.contains(event.target as Node)) setCoverContextMenu(null);
    };
    if (showSettings || coverContextMenu) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [showSettings, coverContextMenu]);

  useEffect(() => {
  }, [bitness, gpuArch]);

  useEffect(() => {
    if (bitness === "32" && route === "optiscaler") {
      changeInstallRoute("reshade"); // Fallback to reshade
    }
  }, [bitness, route]);

  const installationRequest = useRef(0);
  const refreshInstallationDetails = async (path: string) => {
    const request = ++installationRequest.current;
    try {
      const result = await invoke<{ status: string; installed_dll: string | null; dlssnr_installed: boolean }>("get_game_installation_details", { gameDir: path });
      if (request === installationRequest.current) { setInstallStatus(result.status); setInstalledDll(result.installed_dll); setDlssNrInstalled(result.dlssnr_installed); }
    } catch (error) {
      console.error("Could not refresh installation details:", error);
      if (request === installationRequest.current) { setInstallStatus("Erro"); setInstalledDll(null); setDlssNrInstalled(false); }
    }
  };

  const handleInstall = async () => {
    if (gpuArch === "rdna3" && bitness === "32") return;
    if (!gameDir) {
      setShowInstructionsModal(true);
      return;
    }
    setModelRecovery(false);
    if (!appConfig?.dll_version) {
      setModalTitle(t("app", "installFailTitle"));
      setModalMessage(t("installation", "missingModel"));
      setLogs("");
      setModalType("error");
      setModelRecovery(true);
      setShowModal(true);
      return;
    }

    setLoading(true);
    setLoadingMessage(t("app", "installingMsg"));
    setLogs(t("app", "installStarted"));

    // Give browser time to paint the loading modal
    await new Promise(resolve => setTimeout(resolve, 50));

    try {
      const response = await runInstallation({
        gameDir,
        route,
        bitness,
        gpuArch,
        dllPath: /\.(dll|zip)$/i.test(appConfig.dll_version) ? appConfig.dll_version : null,
        binPath: appConfig.dll_version.endsWith('.bin') ? appConfig.dll_version : null,
        shortcutKey: appConfig?.game_shortcut_keys?.[selectedGame?.path || gameDir] || appConfig?.game_shortcut_keys?.[gameDir] || defaultShortcutForRoute(route),
        neuralStartup: neuralStartup.enabled,
        model: neuralModel,
        int4: int4Mixed,
        renodx: renodxStyle,
      });
      setLogs((prev) => prev + "\n" + localizeInstallationMessage(response, language));


      const isRepair = isInstalledMod(installStatus);
      setModalTitle(isRepair ? t("app", "repairSuccessTitle") : t("app", "installSuccessTitle"));
      const printed = launchOptionsFromInstallOutput(response);
      const launchOptions = printed ?? steamLaunchOptionsForRoute(route);
      const layersNote = launchOptions.includes("VK_INSTANCE_LAYERS") ? `\n\n${t("routeSelector", "layersNote")}` : "";
      // The options the mod needs go into the game's own launch options by themselves.
      const target = selectedGame && (selectedGameDirectory === gameDir || selectedGame.path === gameDir) ? selectedGame : null;
      const synced = await syncModLaunchOptions(target, gameDir, launchOptions).catch(error => { notifyError(selectedGame?.name || "DLSS 5 NR", error); return undefined; });
      if (synced) setSelectedGame(previous => previous && previous.path === synced.path ? synced : previous);
      const intro = isRepair ? t("app", "repairSuccessMsg") : t("app", "installSuccessMsg").split("\n\n")[0];
      const appliedKey = synced?.steamModOptions === "written" ? "launchOptionsSteam" : synced?.steamModOptions === "waiting" ? "launchOptionsSteamWaiting" : "launchOptionsApplied";
      // With Steam open, pasting into its launch options applies at once: the line goes to the clipboard.
      if (synced?.steamModOptions === "waiting") void invoke("plugin:clipboard-manager|write_text", { text: launchOptions }).catch(() => {});
      setModalMessage(synced ? `${intro}\n\n${t("app", appliedKey).replace("{launchOptions}", launchOptions)}`
        : synced === undefined ? `${intro}\n\n${t("app", "launchOptionsError").replace("{launchOptions}", launchOptions)}`
        : isRepair ? t("app", "repairSuccessMsg") + (printed ? `\n\n${launchOptions}${layersNote}` : "") : t("app", "installSuccessMsg").replace("{launchOptions}", launchOptions) + layersNote);
      setModalType("success");
      setShowModal(true);
    } catch (err) {
      setLogs((prev) => prev + t("app", "criticalError") + localizeInstallationMessage(String(err), language));

      let shortErr = String(err);
      if (shortErr.includes("Log completo:")) {
          shortErr = shortErr.split("Log completo:")[0].trim();
      } else {
          const lines = shortErr.split("\n");
          shortErr = lines.slice(0, 2).join("\n") + (lines.length > 2 ? "\n..." : "");
      }

      setModalTitle(t("app", "installFailTitle"));
      const missingModel = isMissingModel(err);
      setModelRecovery(missingModel);
      setModalMessage(missingModel ? t("installation", "missingModel") : t("app", "installFailMsg") + localizeInstallationMessage(shortErr, language));
      setModalType("error");
      setShowModal(true);
    } finally {
      // The DLSS view checks the executable's folder, which can differ from the game's path.
      if (selectedGame && (selectedGameDirectory === gameDir || selectedGame.path === gameDir)) {
        await refreshInstallationDetails(gameDir);
        setGameInfoRevision(value => value + 1);
      }
      setLoading(false);
      setLoadingMessage("");
    }
  };

  const [showConfirmGameUninstall, setShowConfirmGameUninstall] = useState<string | null>(null);

  const proceedUninstallForPath = async (path: string) => {
    setModelRecovery(false);
    setLoading(true);
    setLoadingMessage(t("app", "uninstallingMsg"));
    setLogs(t("app", "uninstallStarted"));

    await new Promise(resolve => setTimeout(resolve, 50));

    try {
      const response = await runInstallation({
        gameDir: path,
        route: "remove",
        bitness,
        gpuArch,
        dllPath: null,
        binPath: null,
      });
      setLogs((prev) => prev + "\n" + localizeInstallationMessage(response, language));

      const target = selectedGame && (selectedGameDirectory === path || selectedGame.path === path) ? selectedGame : null;
      const synced = await syncModLaunchOptions(target, path, null).catch(error => { notifyError(selectedGame?.name || "DLSS 5 NR", error); return null; });
      if (synced) setSelectedGame(previous => previous && previous.path === synced.path ? synced : previous);
      setModalTitle(t("app", "uninstallSuccessTitle"));
      setModalMessage(t("app", "uninstallSuccessMsg") + path + (synced?.steamModOptions ? `\n\n${t("app", "launchOptionsRemovedSteam")}` : synced && synced.advanced?.launch_options !== target?.advanced?.launch_options ? `\n\n${t("app", "launchOptionsRemoved")}` : ""));
      setModalType("success");
      setShowModal(true);
    } catch (err) {
      setLogs((prev) => prev + t("app", "criticalError") + localizeInstallationMessage(String(err), language));

      let shortErr = String(err);
      if (shortErr.includes("Log completo:")) {
          shortErr = shortErr.split("Log completo:")[0].trim();
      } else {
          const lines = shortErr.split("\n");
          shortErr = lines.slice(0, 2).join("\n") + (lines.length > 2 ? "\n..." : "");
      }

      setModalTitle(t("app", "uninstallFailTitle"));
      setModalMessage(`${t("app", "uninstallFailMsg")} ${path}${t("app", "uninstallFailLog")}${localizeInstallationMessage(shortErr, language)}`);
      setModalType("error");
      setShowModal(true);
    } finally {
      setLoading(false);
      setLoadingMessage("");
      // Refresh status if selected game was uninstalled
      if (selectedGame && (selectedGameDirectory === path || selectedGame.path === path)) {
        await refreshInstallationDetails(path);
        setGameInfoRevision(value => value + 1);
      }
    }
  };

  const [selectedGame, setSelectedGame] = useState<GameInfo | null>(null);

  const nexus = useNexusWorkspace();
  const nexusText = useNexusText();
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
  const [nexusAccountConnected, setNexusAccountConnected] = useState<boolean | null>(null);
  useEffect(() => {
    if (gamePanelMode !== "nexus" || showSettingsModal) return;
    let active = true;
    void invoke<unknown | null>("load_nexus_account").then(account => { if (active) setNexusAccountConnected(Boolean(account)); })
      .catch(() => { if (active) setNexusAccountConnected(null); });
    return () => { active = false; };
  }, [gamePanelMode, showSettingsModal]);
  useEffect(() => {
    if (gamePanelMode !== "mods" || !selectedGame || !appConfig?.dll_version || loading || showSetupWizard || showSettingsModal) return;
    let active = true, revision = 0;
    const refreshModels = async () => {
      const current = ++revision;
      setNeuralModelsError(false);
      try {
        const status = await invoke<{small:boolean; original:boolean; selectable:boolean}>("get_neural_model_status", {
          gpuArch, bitness, binPath: appConfig.dll_version.endsWith(".bin") ? appConfig.dll_version : null,
        });
        if (!active || revision !== current) return;
        setNeuralModels(status);
        if (!status.small || !status.selectable) setNeuralModel("original");
      } catch {
        if (!active || revision !== current) return;
        setNeuralModels(null); setNeuralModelsError(true);
      }
    };
    const onFocus = () => { void refreshModels(); };
    const onBackendChanged = (event: Event) => {
      const target = (event as CustomEvent<{target?:string}>).detail?.target;
      if (!target || target === gpuArch) void refreshModels();
    };
    setNeuralModels(null);
    void refreshModels();
    window.addEventListener("focus", onFocus);
    window.addEventListener("backendVersionChecked", onBackendChanged);
    return () => {
      active = false; ++revision;
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("backendVersionChecked", onBackendChanged);
    };
    // A new extraction can keep exactly the same dll_version pathname. Recheck
    // the saved configuration object and dialog lifecycle, not just that string.
  }, [gamePanelMode, selectedGame?.path, appConfig, gpuArch, bitness, loading, showSetupWizard, showSettingsModal]);
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
  const [installationExecutable, setInstallationExecutable] = useState("");
  const [installationProton, setInstallationProton] = useState("");
  const gameInstaller = useGameInstaller();
  const gameExecution = useGameExecution();
  const sessionState = gameExecution.session?.state;
  useEffect(() => {
    if (!selectedGame) return;
    let alive = true;
    void loadGameActivity().then(value => { if (alive) setGameActivity(value); });
    return () => { alive = false; };
  }, [selectedGame?.path, sessionState]);
  // Launcher errors also go to the notification center.
  useEffect(() => { if (gameExecution.error) notifyError(language === "en" ? "Game execution" : "Execução do jogo", gameExecution.error); }, [gameExecution.error]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (libraryMutationError) notifyError(language === "en" ? "Library" : "Biblioteca", libraryMutationError); }, [libraryMutationError]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (executableChoiceError) notifyError(language === "en" ? "Game executable" : "Executável do jogo", executableChoiceError); }, [executableChoiceError]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (launcherGamesError) notifyError(language === "en" ? "Launcher games" : "Jogos do launcher", launcherGamesError); }, [launcherGamesError]); // eslint-disable-line react-hooks/exhaustive-deps
  // When a library program closes, games its launcher installed in the prefix are offered.
  const lastExecution = useRef<{ id: string; path: string; active: boolean } | null>(null);
  useEffect(() => {
    const session = gameExecution.session;
    if (!session) return;
    const previous = lastExecution.current;
    lastExecution.current = { id: session.id, path: session.path, active: gameExecution.active };
    if (previous?.id === session.id && previous.active && !gameExecution.active) {
      void newLauncherGames(session.path).then(games => {
        if (games.length) { setLauncherGamesError(""); setLauncherGames({ path: session.path, games }); }
      }).catch(console.error);
    }
  }, [gameExecution.session?.id, gameExecution.session?.state]); // eslint-disable-line react-hooks/exhaustive-deps
  const installationReady = Boolean(!gameExecution.active && !gameExecution.pendingPath && installationDraft.name && !installationDraft.editing && installationLocation.selected && installationLocation.directory.trim() && installationExecutable.trim() && (addingNexusGame || installationProton));
  const [isCollapsingGame, setIsCollapsingGame] = useState(false);
  const detailsOpen = !isCollapsingGame && ((gamePanelMode === "install" || gamePanelMode === "add") ? installationOpen : gamePanelMode === "installed" ? Boolean(selectedGame) : gamePanelMode === "nexus" ? modManagerOpen && Boolean(selectedGame) : modManagerOpen);
  // Shortcut destinations temporarily unmount details; keep the origin tab here.
  const detailsResetKey = `${gamePanelMode}:${selectedGame ? gameViewKey(selectedGame) : "none"}`;
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
      setGamePanelMode("mods");
      setLibraryScope("all");
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
    setShowGameEntryModal(true); return;
    collapseGameDetailsNow();
    nexus.setError("");
    setAddingNexusGame(true); setNexusAdditionPrefix(""); setNexusAdditionPlatform("proton");
    setGamePanelMode("add"); setLibraryScope("all"); setInstallationOpen(true);
    setIsCollapsingGame(false); setPlayGameEntrance(true); setLibraryMutationError("");
  });
  const openNexusLibrary = () => requestDraftExit(() => transitionLibrary(() => {
    collapseGameDetailsNow();
    setGamePanelMode("nexus"); setLibraryScope("all");
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
  const sameFolder = (value?: string) => value?.replace(/\\/g, "/").replace(/\/+$/, "");
  const nexusCandidates = (nexus.loaded ? nexus.workspace.games : cachedNexus.games).filter(item => selectedGame && (
    selectedGame.nexus_id ? item.id === selectedGame.nexus_id :
    sameFolder(item.game.directory || item.game.path) === sameFolder(selectedGame.directory || selectedGame.path)
  ));
  const nexusGame = gamePanelMode === "mods" ? undefined
    : nexusCandidates.find(item => item.id === selectedGame?.nexus_id || (item.game.path === selectedGame?.path && item.game.launcher === selectedGame?.launcher)) ?? (nexusCandidates.length === 1 ? nexusCandidates[0] : undefined);
  const nexusToolState = useNexusGameTools(gamePanelMode === "nexus" ? nexusGame : undefined);
  const [selectedGameCoverError, setSelectedGameCoverError] = useState(false);
  useEffect(() => {
    const coverChanged = (event: Event) => {
      const game = (event as CustomEvent<GameInfo>).detail;
      setSelectedGame(previous => previous && gameViewKey({ ...previous, library_view: gamePanelMode === "nexus" ? "nexus" : libraryScope }) === gameViewKey(game) ? { ...previous, cover_url: game.cover_url, automatic_cover_url: game.automatic_cover_url } : previous);
      setSelectedGameCoverError(false);
    };
    const coverError = () => {
      setModalTitle(t("gameGrid", "coverErrorTitle"));
      setModalMessage(t("gameGrid", "coverError"));
      setModalType("error"); setModelRecovery(false); setLogs(""); setShowModal(true);
    };
    window.addEventListener("gameCoverChanged", coverChanged);
    window.addEventListener("gameCoverError", coverError);
    return () => { window.removeEventListener("gameCoverChanged", coverChanged); window.removeEventListener("gameCoverError", coverError); };
  }, [language, gamePanelMode, libraryScope]);
  const [releaseDate, setReleaseDate] = useState<string>("Desconhecido");
  const [gameInfoRevision, setGameInfoRevision] = useState(0);
  useEffect(() => {
    const refresh = () => setGameInfoRevision(revision => revision + 1);
    window.addEventListener("gameInfoCacheCleared", refresh);
    return () => window.removeEventListener("gameInfoCacheCleared", refresh);
  }, []);
  const selectedGameDirectory = selectedGame?.directory ?? selectedGame?.path;
  const analysis = useGameAnalysis(gamePanelMode === "mods" ? selectedGameDirectory : undefined, selectedGame?.name, selectedGame?.app_id || undefined, gameInfoRevision);
  // New interface: the overview shows the same read-only analysis for every library.
  const overviewAnalysis = useGameAnalysis(selectedGame && !["mods", "install", "add"].includes(gamePanelMode) ? selectedGameDirectory : undefined, selectedGame?.name, selectedGame?.app_id || undefined, gameInfoRevision);
  const upscalerFamilies = [...new Set((analysis.upscalers ?? []).map(name => {
    const family = name.trim().match(/^(DLSS|FSR|XeSS)(?=$|[\s\d._-])/i)?.[1]?.toLowerCase();
    return family === "dlss" ? "DLSS" : family === "fsr" ? "FSR" : family === "xess" ? "XeSS" : name;
  }))];
  const manualRoutes = useRef(new Map<string, InstallRoute>());
  const detectedBitness = analysis.architecture === "32-bits" ? "32" : analysis.architecture === "64-bits" ? "64" : null;
  useEffect(() => {
    if (detectedBitness) setBitness(detectedBitness);
  }, [selectedGame?.path, detectedBitness, gameInfoRevision]);
  const [dlssNrInstalled, setDlssNrInstalled] = useState(false);
  const [installStatus, setInstallStatus] = useState<string>("Verificando...");
  const recommendOptiscaler = detectedBitness === "64" && bitness === "64" && upscalerFamilies.some(name => ["DLSS", "FSR", "XeSS"].includes(name));
  useEffect(() => {
    if (selectedGame && recommendOptiscaler && !manualRoutes.current.has(selectedGame.path)) changeInstallRoute("optiscaler");
  }, [selectedGame?.path, recommendOptiscaler]);
  const neuralStartup = useNeuralStartup(gamePanelMode === "mods" ? selectedGameDirectory : undefined, gamePanelMode === "mods" && isInstalledMod(installStatus));
  const titlebarRunningGames = useTitlebarRunningGames({execution:gameExecution, selected:selectedGame,
    source:gamePanelMode === "mods" || libraryScope === "all" ? "dlss" : "main",
    nexusGames:nexus.workspace.games, nexusLaunching:nexus.launching, nexusStopping:nexus.stopping, nexusBusy:nexus.busy, stopNexus:nexus.stop, onDetails:(game, origin)=>handleSelectGame(game, undefined, true, origin)});
  const runningGameTimes = useRunningGameTimes(titlebarRunningGames);
  useMatrixGameActivity(titlebarRunningGames);
  const [installedDll, setInstalledDll] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const request = ++installationRequest.current;
    if (selectedGame) {
      setSelectedGameCoverError(false);
      if (selectedGame.app_id) {
        setReleaseDate("Detectando...");
        invoke<string>("fetch_steam_release_date", { appId: selectedGame.app_id })
          .then((date: string) => { if (active) setReleaseDate(date); })
          .catch(() => { if (active) setReleaseDate("Desconhecido"); });
      } else {
        setReleaseDate("Desconhecido");
      }

      setInstallStatus("Nenhum");
      setInstalledDll(null); setDlssNrInstalled(false);
      setInstallStatus("Verificando...");
      invoke<{ status: string; installed_dll: string | null; dlssnr_installed: boolean }>("get_game_installation_details", { gameDir: selectedGameDirectory })
        .then(result => {
          if (active && request === installationRequest.current) { setInstallStatus(result.status); setInstalledDll(result.installed_dll); setDlssNrInstalled(result.dlssnr_installed); }
        })
        .catch(() => { if (active && request === installationRequest.current) { setInstallStatus("Erro"); setInstalledDll(null); setDlssNrInstalled(false); } });

    } else {
      setReleaseDate("Desconhecido");
      setInstallStatus("Nenhum");
      setInstalledDll(null); setDlssNrInstalled(false);
    }
    return () => { active = false; };
  }, [selectedGame?.path, selectedGame?.directory, selectedGame?.app_id, gameInfoRevision, gamePanelMode]);

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
      setInstallStatus("Verificando...");
      setInstalledDll(null); setDlssNrInstalled(false);
      ++installationRequest.current;
      const scope = origin ? origin === "main" ? "own" : "all" : libraryScope;
      if (origin) { setLibraryScope(scope); setShowWelcome(false); setShowSettings(false); }
      const manualRoute = manualRoutes.current.get(game.path);
      if (scope === "all" && manualRoute) changeInstallRoute(manualRoute);
      setModManagerOpen(true);
      setGamePanelMode(origin === "nexus" || game.library_view === "nexus" ? "nexus" : scope === "all" ? "mods" : "installed");
      setGameDir(game.directory ?? game.path);
      setSelectedGame(game);
    }
  });

  const coverAction = (action: CoverGameAction, game: GameInfo) => {
    setCoverContextMenu(null);
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
      } else if (gameExecution.pendingPath === game.path || (gameExecution.session?.path === game.path && ["starting", "running"].includes(gameExecution.session.state))) return gameExecution.cancel();
      return;
    }
    if (gameInstaller.busy || libraryMutationBusy || gameExecution.active || gameExecution.pendingPath) return;
    if (action === "start") {
      if (game.library_view === "nexus") {
        const entry = nexus.workspace.games.find(item => item.game.path === game.path && item.game.launcher === game.launcher);
        if (entry) return entry.running ? nexus.stop(entry.id) : nexus.start(entry.id);
        return;
      }
      if (game.launcher === "PeliGames") return gameExecution.start(game.path);
      // Launch errors go to the bell only: the execution state already reports a Steam failure there.
      const execution = language === "en" ? "Game execution" : "Execução do jogo";
      if (game.launcher === "Steam") return game.app_id ? gameExecution.startSteam(game.path, game.app_id, game.external_id || undefined).catch(() => {}) : notifyError(execution, language === "en" ? "Steam AppID unavailable." : "Steam AppID indisponível.");
      else if (game.external_id) return gameExecution.startExternal(game.path, game.external_id).catch(() => {});
    } else if (game.launcher === "PeliGames") setUninstallTarget({path: game.path, name: game.name, prefix: game.prefix || ""});
    else if (game.launcher === "Steam") void invoke("request_steam_uninstall", { appId: game.app_id }).catch(error => notifyError(language === "en" ? "Uninstall" : "Desinstalação", error));
  };

  const changeSelectedExecutable = () => {
    if (!selectedGame || executableAlternatives.length < 2 || libraryMutationBusy || gameInstaller.busy || gameExecution.active || gameExecution.pendingPath) return;
    setExecutableChoiceError("");
    setExecutableChoice({ entries: executableAlternatives, prefix: selectedGame.prefix || "", changing: true });
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
        else if (found.length > 1) { setExecutableChoiceError(""); setExecutableChoice({ entries: found, prefix: result.prefix }); }
        else if (found[0]) showCompletedGameEntry(found[0]);
      }
    }
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

  const detailHoverFrame = useRef<number | null>(null);
  const detailHoverBounds = useRef<{ card: HTMLDivElement; rect: DOMRect } | null>(null);
  const detailHoverPoint = useRef({ x: 0, y: 0 });
  useEffect(() => () => {
    if (detailHoverFrame.current !== null) cancelAnimationFrame(detailHoverFrame.current);
    detailHoverFrame.current = null;
    detailHoverBounds.current = null;
  }, [performanceMode, selectedGame?.path]);
  const handleCoverMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    if (performanceMode) return;
    const card = e.currentTarget;
    if (detailHoverBounds.current?.card !== card) detailHoverBounds.current = { card, rect: card.getBoundingClientRect() };
    detailHoverPoint.current = { x: e.clientX, y: e.clientY };
    if (detailHoverFrame.current !== null) return;
    detailHoverFrame.current = requestAnimationFrame(() => {
      detailHoverFrame.current = null;
      const bounds = detailHoverBounds.current;
      if (!bounds || bounds.card !== card || !card.isConnected) return;
      const { rect } = bounds;
      if (!rect.width || !rect.height) return;
      const px = Math.max(0, Math.min(1, (detailHoverPoint.current.x - rect.left) / rect.width));
      const py = Math.max(0, Math.min(1, (detailHoverPoint.current.y - rect.top) / rect.height));
      card.style.transition = "transform .12s ease-out, box-shadow .25s ease";
      card.style.transform = `perspective(900px) rotateX(${(py * 2 - 1) * -6}deg) rotateY(${(px * 2 - 1) * 6}deg) scale3d(1.04, 1.04, 1.04)`;
      const glare = card.querySelector<HTMLElement>('.glare');
      if (glare) {
        glare.style.setProperty("--glare-x", `${px * 100}%`);
        glare.style.setProperty("--glare-y", `${py * 100}%`);
        glare.style.setProperty("--glare-sheen", `${120 - (px * 70 + py * 30) * 1.4}%`);
        glare.style.opacity = "1";
      }
    });
  };

  const handleCoverMouseLeave = (e: React.MouseEvent<HTMLDivElement>) => {
    if (detailHoverFrame.current !== null) cancelAnimationFrame(detailHoverFrame.current);
    detailHoverFrame.current = null;
    detailHoverBounds.current = null;
    const card = e.currentTarget;
    card.style.transition = "transform .45s cubic-bezier(.2, .8, .2, 1), box-shadow .35s ease";
    card.style.transform = "none";
    const glare = card.querySelector<HTMLElement>('.glare');
    if (glare) glare.style.opacity = "0";
  };

  const handleChooseCustomPath = async () => {
    if (!selectedGame || directoryPickerBusy) return;
    const game = selectedGame;
    setDirectoryPickerBusy(true);
    try {
      const path = await openDirectoryPicker(t("gameInfo", "editDirectory"), selectedGameDirectory || game.path);
      if (!path || path === (selectedGameDirectory || game.path)) return;
      await invoke("save_custom_game_path", { gameName: game.name, newPath: path });
      window.dispatchEvent(new CustomEvent("refreshGames", { detail: { oldPath: game.path, newPath: path } }));
      setSelectedGame(prev => prev?.path === game.path ? { ...prev, path } : prev);
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
  const hasBlockingDialog = Boolean(droppedPath) || hasOverlayDialog || showDiscardDraft || showGameEntryModal || Boolean(uninstallTarget) || Boolean(gameLogsTarget) || showWelcome || Boolean(appImageTarget) || showModManagerModal || showAppUpdate || showSetupWizard || showCreditsModal || showInstructionsModal || showUpdaterModal || showModal || !!showConfirmGameUninstall || loading;
  const hasOpenDialog = (detailsOpen && gameDetailsOverlay && !showSettingsModal) || hasBlockingDialog;
  const fileDropHover = useLauncherFileDrop(hasOpenDialog || preferencesBusy || gameInstaller.busy || libraryMutationBusy || nexus.busy || hasUnfinishedDraft, setDroppedPath);

  useEffect(() => {
    if (hasOpenDialog) {
      setShowSettings(false);
      setShowLangDropdown(false);
      setCoverContextMenu(null);
    }
  }, [hasOpenDialog]);

  useEffect(() => {
    const openCoverSettings = () => {
      if (hasOpenDialog) return;
      setSettingsSection("covers");
      setShowSettingsModal(true);
    };
    const openProtonSettings = () => {
      // This action originates inside game details; that panel must not block
      // its own route to runner management. Keep unrelated dialogs protected.
      if (hasBlockingDialog || preferencesBusy || gameInstaller.busy || libraryMutationBusy || nexus.busy || nexus.settingsBusy) return;
      setShowSettings(false);
      setSettingsSection("protons");
      setSettingsPage("protons");
      setShowSettingsModal(true);
    };
    const openMonitoringSettings = () => {
      if (loading || preferencesBusy || showDiscardDraft) return;
      setSettingsPage("monitoring");
      setSettingsSection("monitoring");
      setShowSettingsModal(true);
    };
    window.addEventListener("openMonitoringPreferences", openMonitoringSettings);
    window.addEventListener("openCoverPreferences", openCoverSettings);
    window.addEventListener("openProtonPreferences", openProtonSettings);
    return () => {
      window.removeEventListener("openMonitoringPreferences", openMonitoringSettings);
      window.removeEventListener("openCoverPreferences", openCoverSettings);
      window.removeEventListener("openProtonPreferences", openProtonSettings);
    };
  }, [hasOpenDialog, hasBlockingDialog, preferencesBusy, gameInstaller.busy, libraryMutationBusy, nexus.busy, nexus.settingsBusy, loading, showSettingsModal, showDiscardDraft]);


  // Pieces of the selected-game area.
  const viewDownloadedMod = (gameId: string, modId: string) => requestDraftExit(() => { const entry = nexus.workspace.games.find(item => item.id === gameId); if (!entry) return; setNexusModToView({gameId,modId,requestId: ++modViewRequest.current}); setNexusTab("mods"); setShowWelcome(false); setGamePanelMode("nexus"); setLibraryScope("all"); setModManagerOpen(true); setIsCollapsingGame(false); setSelectedGame({ ...entry.game, nexus_platform: entry.platform, nexus_id: entry.id }); setGameDir(entry.game.directory || entry.game.path); });
  const detailsSummary = (gamePanelMode === "install" || gamePanelMode === "add") ? <InstallGameSummary nexusAddition={addingNexusGame} mode={gamePanelMode} draft={installationDraft} ready={installationReady} busy={gameInstaller.busy || libraryMutationBusy || (addingNexusGame && nexus.busy)} installationResult={gameInstaller.result} installationError={libraryMutationError || (addingNexusGame ? nexus.error : gameInstaller.error)}
              onInstall={() => void submitGameEntry()}
              onUninstall={gameInstaller.result ? () => setUninstallTarget({ path: gameInstaller.result!.prefix.replace(/[\\/]prefix[\\/]?$/, ""), prefix: gameInstaller.result!.prefix, name: installationDraft.name }) : undefined}
              onCollapse={collapseGameDetails} onCoverMove={handleCoverMouseMove} onCoverLeave={handleCoverMouseLeave} reducedMotion={performanceMode} /> : gamePanelMode === "nexus" && !selectedGame ? <NexusEmptySummary onAdd={openNexusAddition} onCollapse={collapseGameDetails} loaded={nexus.loaded} busy={nexus.busy} reducedMotion={performanceMode} onCoverMove={handleCoverMouseMove} onCoverLeave={handleCoverMouseLeave} /> : !selectedGame ? <InstallGameSummary draft={installationDraft} mode="mods" onCollapse={collapseGameDetails} onCoverMove={handleCoverMouseMove} onCoverLeave={handleCoverMouseLeave} reducedMotion={performanceMode} /> : null;
  const selectedCover = selectedGame ? (
                <motion.div 
                  key={`${gameViewKey(selectedGame)}-${performanceMode ? "performance" : "elegant"}`}
                  className={`selected-cover ${gamePanelMode === "nexus"
                    ? nexusGame && nexus.launching === nexusGame.id && nexus.stopping !== nexusGame.id && !nexusGame.running ? "is-preparing-game" : ""
                    : (gameExecution.pendingPath === selectedGame.path || (gameExecution.session?.path === selectedGame.path && gameExecution.session.state === "starting")) && !(gameExecution.session?.path === selectedGame.path && gameExecution.session.state === "running") ? "is-preparing-game" : ""}`}
                  ref={coverFlight.destinationRef}
                  layout={false}
                  onMouseMove={handleCoverMouseMove}
                  onMouseLeave={handleCoverMouseLeave}
                  style={{ 
                    position: "relative",
                    borderRadius: "12px", 
                    overflow: "hidden",
                    transition: "box-shadow 0.1s ease-out",
                    boxShadow: "0 0 20px rgba(var(--accent-rgb), 0.6)",
                    border: "2px solid rgba(var(--accent-rgb), 0.8)",
                    transformStyle: "flat",
                    willChange: "auto",
                    cursor: "pointer",
                    height: "max-content",
                    display: "inline-block"
                  }}
                  onContextMenu={(e) => { e.preventDefault(); setCoverContextMenu({ x: e.clientX, y: e.clientY }); }}
                >
                  <GamePlatformBadge game={selectedGame} platform={gamePanelMode === "nexus" ? nexusGame?.platform : undefined} />
                  <div className="glare" style={{
                    position: "absolute", top: 0, left: 0, right: 0, bottom: 0,
                    pointerEvents: "none", opacity: 0, transition: "opacity 0.2s ease-out", zIndex: 10
                  }} />
                  {selectedGame.cover_url && !selectedGameCoverError ? (
                    <img 
                      src={selectedGame.cover_url} 
                      alt={selectedGame.name}
                      decoding="async"
                      onError={() => setSelectedGameCoverError(true)}
                      style={{ 
                        width: "180px", 
                        objectFit: "cover",
                        aspectRatio: "2/3",
                        display: "block"
                      }} 
                    />
                  ) : (
                    <div style={{ 
                        width: "180px", aspectRatio: "2/3", 
                        background: "linear-gradient(135deg, #1e3a8a, #312e81)", 
                        position: "relative",
                        textAlign: "center"
                      }}>
                      <span style={{ position: "absolute", top: "1.5rem", left: "1rem", right: "1rem", fontWeight: "bold", fontSize: "1.1rem", textShadow: "0 2px 4px rgba(0,0,0,0.8)" }}>{selectedGame.name}</span>
                      <img src="/peligames.svg" alt="Icon" style={{ position: "absolute", top: "50%", left: "50%", transform: "translate(-50%, -50%)", width: "60px", height: "60px", opacity: 0.5 }} />
                    </div>
                  )}
                </motion.div>
  ) : null;
  const nexusPanels = <NexusPanels toolState={gamePanelMode === "nexus" ? nexusToolState : undefined} highlightRequestId={nexusModToView?.requestId} highlightedModId={nexusModToView?.gameId === nexusGame?.id ? nexusModToView?.modId : undefined} tab={nexusTab} onTabChange={tab => { if (tab === "mods") setNexusTab(tab); else void requireNexusAccount(language).then(connected => { if (connected) setNexusTab(tab); }); }} game={nexusGame} busy={nexus.busy} settingsBusy={nexus.settingsBusy} progress={nexusGame ? nexus.progress[nexusGame.id] : undefined} error={nexus.error} reducedMotion={performanceMode} onImport={nexus.importArchive} onRemove={nexus.removeArchive} onInstall={nexus.install} onToggle={nexus.toggle} onMove={nexus.move} onSwitchProfile={nexus.switchProfile} onDownloadLoader={nexus.downloadLoader} onLogs={setNexusLogTarget} onConfigure={nexus.configureProton} onConfigureDeploy={nexus.configureDeploy} onConfigureLaunch={nexus.configureLaunch} onUninstall={nexusGame && ["PeliGames", "Steam"].includes(nexusGame.game.launcher) ? () => {void coverAction("uninstall", {...nexusGame.game, prefix:nexusGame.compat_data || nexusGame.game.prefix});} : undefined} onConfigureAdvanced={nexus.configureAdvanced} onError={nexus.setError} />;
  const installedPanel = (section: InstalledPanelSection) => <InstalledGamePanels key={selectedGame?.path} section={section} game={selectedGame} onChangeExecutable={selectedGame?.launcher === "PeliGames" && executableAlternatives.length > 1 ? changeSelectedExecutable : undefined} onUninstall={selectedGame && ["PeliGames", "Steam"].includes(selectedGame.launcher) ? () => {void coverAction("uninstall", selectedGame);} : undefined} disabled={gameInstaller.busy || libraryMutationBusy || gameExecution.active || Boolean(gameExecution.pendingPath)}
              onSaved={game => { setSelectedGame(previous => previous && gameViewKey({ ...previous, library_view: gamePanelMode === "nexus" ? "nexus" : libraryScope }) === gameViewKey(game) ? game : previous); }} programsDisabled={gameInstaller.busy || libraryMutationBusy || Boolean(gameExecution.pendingPath)}
              onRunProgram={gameExecution.runProgram} onStartShortcut={gameExecution.startShortcut} />;
  const installationPanels = <InstallGamePanels primaryAction={<InstallGameSubmit mode={gamePanelMode === "add" ? "add" : "install"} ready={installationReady} busy={gameInstaller.busy || libraryMutationBusy || (addingNexusGame && nexus.busy)} onInstall={() => void submitGameEntry()} />} onClose={collapseGameDetails} mode={gamePanelMode === "add" ? "add" : "install"} draft={installationDraft} directory={installationLocation.directory} directorySelected={installationLocation.selected} onDefaultDirectory={installationLocation.selectDefault} directoryError={installationLocation.error} busy={gameInstaller.busy || libraryMutationBusy || (addingNexusGame && nexus.busy)} nexusAddition={addingNexusGame ? { platform: nexusAdditionPlatform, onPlatformChange: setNexusAdditionPlatform, prefix: nexusAdditionPrefix, onPrefixChange: setNexusAdditionPrefix } : undefined} onDirectoryChange={installationLocation.setDirectory}
                executable={installationExecutable} onExecutableChange={setInstallationExecutable} proton={installationProton} onProtonChange={setInstallationProton} />;
  // ---- New interface (Settings → Interface → layout) ----
  const modernSection: ModernSection = showSettingsModal ? "settings" : gamePanelMode === "nexus" || addingNexusGame ? "nexus"
    : gamePanelMode === "mods" ? "dlss"
    : (gamePanelMode === "install" || gamePanelMode === "add") && installationOpen ? "install" : "library";
  const navigateModern = (section: ModernSection) => {
    if (section === "discord") { window.dispatchEvent(new Event(OPEN_DISCORD_PANEL_EVENT)); return; }
    if (section === "community") { window.dispatchEvent(new Event(OPEN_MATRIX_CHAT_EVENT)); return; }
    if (section === "donate") { void openUrl(SUPPORT_PROJECT_URL).catch(error => notifyError(t("credits", "support"), String(error))); return; }
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
  // Overview tiles: what the launcher knows about the game at a glance.
  const info = gamePanelMode === "mods" ? analysis : overviewAnalysis;
  const infoFamilies = [...new Set((info.upscalers ?? []).map(name => {
    const family = name.trim().match(/^(DLSS|FSR|XeSS)(?=$|[\s\d._-])/i)?.[1]?.toLowerCase();
    return family === "dlss" ? "DLSS" : family === "fsr" ? "FSR" : family === "xess" ? "XeSS" : name;
  }))];
  const detectedInfo = (value?: string | null) => {
    const text = value?.trim().normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/[.…]+$/g, "").trim();
    return Boolean(text && !/^(?:[-—–]|n\/a|unknown|none|not (?:detected|identified|found)|nao (?:detectad[oa]|identificad[oa]|encontrad[oa])|desconhecid[oa]|nenhum[a]?|verificando|detectando|checking|detecting|loading|carregando)$/.test(text));
  };
  const checking = (value: string) => value === "Verificando..." ? "…" : localizeAnalysisValue(value);
  const selectedProton = gamePanelMode === "nexus" ? nexusGame?.proton : selectedGame?.proton;
  const nexusPlatform = nexusGame ? (nexusGame.platform === "native" ? t("modern", "nativeLinux") : "Windows") : "";
  const tile = (label: string, value: ReactNode, wide = false) => <div className={`modern-tile${wide ? " wide" : ""}`}><small>{label}</small><strong>{value}</strong></div>;
  const openSelectedFolder = () => invoke("open_folder", { path: selectedGameDirectory }).catch((error: unknown) => {
    setModalTitle(t("gameInfo", "openFolder")); setModalMessage(String(error)); setModalType("error"); setShowModal(true);
  });
  const overviewPrefix = gamePanelMode === "nexus" ? nexusGame?.compat_data || selectedGame?.prefix : selectedGame?.prefix;
  const overviewExecutable = (gamePanelMode === "nexus" ? nexusGame?.game.executable : selectedGame?.executable)?.trim();
  const overviewExecutableFolder = overviewExecutable && /[\\/]/.test(overviewExecutable)
    ? overviewExecutable.replace(/[\\/][^\\/]*$/, "") || "/" : undefined;
  const pathTile = (label: string, path?: string, action?: ReactNode) => detectedInfo(path) ? <div className="modern-tile"><small>{label}</small>{action ? <div className="modern-path"><code title={path}>{path}</code>{action}</div> : <code title={path}>{path}</code>}</div> : null;
  const overviewTiles = selectedGame ? <div className="modern-tiles modern-overview-tiles">
    {detectedInfo(gamePanelMode === "nexus" ? nexusPlatform : info.platform) && tile(t("modern", "platform"), gamePanelMode === "nexus" ? nexusPlatform : /^windows\b/i.test(info.platform) ? "Windows" : checking(info.platform))}
    {detectedInfo(info.graphics_api) && tile(t("modern", "graphicsApi"), checking(info.graphics_api))}
    {detectedInfo(info.architecture) && tile(t("modern", "architecture"), checking(info.architecture))}
    {infoFamilies.filter(detectedInfo).length > 0 && tile(t("modern", "upscalers"), infoFamilies.filter(detectedInfo).join(" · "))}
    {detectedInfo(selectedProton) && tile("Proton", <span title={selectedProton}>{selectedProton?.split("/").filter(Boolean).pop()}</span>)}
    {detectedInfo(releaseDate) && tile(t("gameInfo", "release"), releaseDate)}
    {isInstalledMod(installStatus) && detectedInfo(installedDll) && tile(t("gameInfo", "injectedDll"), <code>{installedDll}</code>)}
    {gamePanelMode === "mods" && isInstalledMod(installStatus) && tile(t("gameInfo", "modInstalled"), installStatus)}
    {detectedInfo(nexusGame?.profile) && tile(language === "pt" ? "Perfil de mods" : "Mod profile", nexusGame?.profile)}
    <div className="modern-overview-paths">
      {pathTile(t("modern", "gameFolder"), selectedGameDirectory, gamePanelMode === "mods" && <button type="button" className="modern-mini-btn" disabled={directoryPickerBusy} onClick={() => void handleChooseCustomPath()} title={t("gameInfo", "editDirectory")}>{t("modern", "change")}</button>)}
      {pathTile(language === "pt" ? "Pasta do prefixo" : "Prefix folder", overviewPrefix)}
      {pathTile(language === "pt" ? "Pasta do executável" : "Executable folder", overviewExecutableFolder)}
    </div>
  </div> : null;
  // Spotlight actions: one big Play button and quick buttons, wired to the
  // same handlers as the cover menu and the classic panel.
  const selectedView = selectedGame ? { ...selectedGame, library_view: gamePanelMode === "nexus" ? "nexus" as const : gamePanelMode === "mods" ? "all" as const : "own" as const } : null;
  const selectedActive = gamePanelMode === "nexus" ? Boolean(nexusGame && (nexusGame.running || nexus.launching === nexusGame.id))
    : Boolean(selectedGame && (gameExecution.pendingPath === selectedGame.path || (gameExecution.session?.path === selectedGame.path && ["starting", "running", "stopping"].includes(gameExecution.session.state))));
  const selectedLaunchable = Boolean(selectedGame && (gamePanelMode === "nexus" ? nexusGame : selectedGame.launcher === "PeliGames" || selectedGame.launcher === "Steam" || (gamePanelMode === "installed" && selectedGame.external_id)));
  const selectedRunning = gamePanelMode === "nexus" ? Boolean(nexusGame?.running) : Boolean(selectedGame && gameExecution.session?.path === selectedGame.path && gameExecution.session.state === "running");
  const selectedPreparing = selectedActive && !selectedRunning && (gamePanelMode === "nexus" ? nexus.stopping !== nexusGame?.id : gameExecution.session?.state !== "stopping" && gameExecution.cancellingPath !== selectedGame?.path);
  const selectedRunKey = gamePanelMode === "nexus" ? `nexus:${nexusGame?.id}` : `peligames:${gameExecution.session?.id || selectedGame?.path}`;
  const playBlocked = gamePanelMode === "nexus" ? Boolean(nexusGame && (nexus.stopping === nexusGame.id || (nexus.busy && nexus.launching !== nexusGame.id)))
    : (gameExecution.cancellingPath === selectedGame?.path || (gameExecution.session?.path === selectedGame?.path && gameExecution.session?.state === "stopping")) || (!selectedActive && Boolean(gameInstaller.busy || libraryMutationBusy || gameExecution.active || gameExecution.pendingPath));
  const playSelected = () => {
    if (!selectedView) return;
    if (gamePanelMode === "nexus") { if (nexusGame) void (selectedActive ? nexus.stop(nexusGame.id) : nexus.start(nexusGame.id)); return; }
    void coverAction(selectedActive ? "stop" : "start", selectedView);
  };
  const quick = (icon: Parameters<typeof MenuIcon>[0]["name"], label: string, onClick: () => void, options: { pressed?: boolean; disabled?: boolean; danger?: boolean } = {}) =>
    <HoverTooltip key={label} text={label} hoverDelay={300} anchorClassName="modern-quick-anchor"><button type="button" className={`modern-quick-btn${options.danger ? " danger" : ""}`} aria-label={label} aria-pressed={options.pressed} disabled={options.disabled} onClick={onClick}><ModernQuickIcon name={icon} /></button></HoverTooltip>;
  const modernActions = selectedGame && selectedView ? <>
    <SteamPendingActions game={selectedGame} disabled={playBlocked || selectedActive} onSaved={game => setSelectedGame(previous => previous?.path === game.path ? {...previous, ...game} : previous)}>
    <button type="button" className={`btn btn-play game-play-button modern-play${selectedActive ? " running" : ""}${selectedPreparing ? " modern-play-preparing" : ""}`} disabled={!selectedLaunchable || playBlocked} aria-busy={selectedPreparing}
      title={selectedLaunchable ? undefined : "Adicione ao PeliGames para iniciar por aqui."} onClick={playSelected}>
      <MenuIcon name={selectedActive ? "stop" : "play"} />{selectedActive ? t("gameExecution", "cancel") : t("modern", "play")}
      {selectedRunning && runningGameTimes[selectedRunKey] !== undefined && <GameRunTimer startedAt={runningGameTimes[selectedRunKey]} />}
    </button>
    </SteamPendingActions>
    <div className="modern-quick">
      {quick("heart", t("modern", "favorite"), () => toggleFavorite(selectedView), { pressed: favorites.has(gameViewKey(selectedView)) })}
      {quick("folder", t("modern", "openFolder"), () => void openSelectedFolder())}
      {quick("logs", t("modern", "logs"), () => { if (gamePanelMode === "nexus") { if (nexusGame) setNexusLogTarget(nexusGame.id); } else coverAction("logs", selectedView); })}
      {gamePanelMode === "nexus" && nexusGame && quick("bug", t("modern", "reportError"), () => setNexusReportTarget(nexusGame.id))}
      {gamePanelMode === "installed" && selectedGame.launcher === "PeliGames" && quick("trash", t("gameInfo", "uninstall"), () => { void coverAction("uninstall", selectedView); }, {
        danger: true,
        disabled: gameInstaller.busy || libraryMutationBusy || gameExecution.active || Boolean(gameExecution.pendingPath),
      })}
    </div>
  </> : null;
  // DLSS 5 NR tab: install or repair first, then its options.
  const dlssInstallPanel = gamePanelMode === "mods" ? <ModernDlssPanel route={route} bitness={bitness} gpuArch={gpuArch}
    recommendedBitness={detectedBitness} recommendOptiscaler={recommendOptiscaler}
    busy={loading} onRoute={value => { if (selectedGame) manualRoutes.current.set(selectedGame.path, value); changeInstallRoute(value); }} onBitness={setBitness}
    recommendation={recommendOptiscaler ? (language === "pt" ? "Recomendado para este jogo: OptiScaler, pelos upscalers detectados." : "Recommended for this game: OptiScaler, based on the detected upscalers.") : undefined}
    startup={neuralStartup.enabled} startupDisabled={loading || neuralStartup.initializing || installStatus === "Verificando..."}
    onStartup={() => neuralStartup.change(!neuralStartup.enabled)} renodx={renodxStyle} onRenodx={() => setRenodxStyle(value => !value)}
    model={neuralModel} installed={isInstalledMod(installStatus)} modelAvailable={Boolean(neuralModels?.small && neuralModels.original && neuralModels.selectable)}
    modelStatus={neuralModelsError ? "error" : !neuralModels ? "checking" : !neuralModels.selectable ? "backend" : !neuralModels.small ? "missing" : "ready"}
    onModel={value => {
      if (value !== neuralModel && isInstalledMod(installStatus)) {
        notifyInfo("DLSS 5 NR", language === "pt"
          ? "Modelo selecionado. Clique em Reparar / reinstalar para aplicar a troca ao jogo."
          : "Model selected. Click Repair / reinstall to apply the change to the game.");
      }
      setNeuralModel(value); if (value === "small") setInt4Mixed(false);
    }}
    int4={int4Mixed} onInt4={() => setInt4Mixed(value => !value)}
    actions={<>
      <button type="button" className="modern-dlss-primary" disabled={loading || neuralStartup.busy || neuralStartup.initializing || installStatus === "Verificando..." || (gpuArch === "rdna3" && bitness === "32")}
        onClick={() => void handleInstall()}>{loading ? t("installAction", "loading") : isInstalledMod(installStatus) ? (language === "pt" ? "Reparar / reinstalar" : "Repair / reinstall") : t("installAction", "install")}</button>
      {isInstalledMod(installStatus) && <button type="button" className="modern-mini-btn danger" disabled={loading || neuralStartup.busy} onClick={() => setShowConfirmGameUninstall(selectedGameDirectory!)}><MenuIcon name="trash" />{t("gameInfo", "uninstall")}</button>}
    </>}
    shortcut={<ShortcutKeySelector disabled={loading || installStatus === "Verificando..."} route={route} shortcutKey={appConfig?.game_shortcut_keys?.[selectedGame?.path || gameDir] || appConfig?.game_shortcut_keys?.[gameDir] || defaultShortcutForRoute(route)} setShortcutKey={val => {
      if (appConfig && selectedGame) {
        const newConfig = { ...appConfig, game_shortcut_keys: { ...appConfig.game_shortcut_keys, [selectedGame.path]: val } };
        setAppConfig(newConfig);
        invoke("save_app_config", { config: newConfig }).catch(err => console.error("Failed to save config:", err));
        if (isInstalledMod(installStatus)) invoke("update_shortcut_key_in_game", { gameDir: selectedGameDirectory, shortcutKey: val }).catch(err => console.error("Failed to update shortcut in game:", err));
      }
    }} />}
    feedback={<>{neuralStartup.feedback && <p role="status" className="neural-startup-feedback">{t("neuralStartup", neuralStartup.feedback)}</p>}
      {neuralStartup.error && <p role="alert" className="neural-startup-error">{t("neuralStartup", "error")} {neuralStartup.error}</p>}</>}
  /> : null;
  const installedModCount = Math.max(nexusGame?.mods.filter(mod => mod.installed).length ?? 0, nexusGame?.detected_mods?.filter(mod => mod.enabled).length ?? 0);
  const installedModPreview = nexusGame?.mods.filter(mod => mod.installed).slice(0, 10) ?? [];
  const integrationPanel = (kind: "mods" | "dlss") => <div className="modern-integration-panel modern-tile">
    <h3>{kind === "mods" ? t("modern", "mods") : t("modern", "dlssConfig")}</h3>
    {kind === "mods" ? <ul className={`modern-mod-preview${installedModPreview.length > 5 ? " modern-mod-preview-columns" : ""}`}>{installedModPreview.map(mod => <li key={mod.id}>{mod.name}</li>)}</ul> : <p>{installStatus}{installedDll ? " · " + installedDll : ""}</p>}
    <button type="button" className="modern-mini-btn" onClick={() => { if (kind === "mods") { setNexusTab("mods"); void nexus.refresh(); } if (selectedGame) handleSelectGame(kind === "mods" && nexusGame ? nexusGame.game : selectedGame, undefined, true, kind === "mods" ? "nexus" : "dlss"); }}>{language === "pt" ? "Abrir gerenciamento" : "Open management"}</button>
  </div>;
  const supportsExecutionSettings = selectedGame?.launcher === "PeliGames" || Boolean(selectedGame?.external_id);
  const executionPanel = (section: "performance" | "options" | "advanced") => supportsExecutionSettings ? installedPanel(section) : <div className="modern-tile"><h3>{section === "advanced" ? t("installedSettings", "advancedTitle") : t("modern", section)}</h3><p>{language === "pt" ? "As opções de execução deste jogo são gerenciadas pelo launcher de origem." : "This game's launch options are managed by its original launcher."}</p></div>;
  const modernTabs: ModernTab[] = gamePanelMode === "install" || gamePanelMode === "add"
    ? [{ id: "installation", label: t("modern", "installation"), content: <div className="modern-install-panels">{installationPanels}</div> }]
    : gamePanelMode === "mods" ? [
      { id: "dlss", label: t("modern", "dlssConfig"), content: dlssInstallPanel },
      { id: "overview", label: t("modern", "overview"), content: overviewTiles },
      { id: "performance", label: t("modern", "performance"), content: executionPanel("performance") },
      { id: "options", label: t("modern", "options"), content: executionPanel("options") },
      { id: "advanced", label: language === "pt" ? "Avançado" : "Advanced", content: executionPanel("advanced") },
    ]
    : gamePanelMode === "nexus" ? [
      { id: "nexus", label: t("modern", "mods"), count: installedModCount, content: cloneElement(nexusPanels, {section: "mods"}) },
      ...(nexusToolState.available ? [{ id: "tools", label: language === "pt" ? "Ferramentas" : "Tools", content: cloneElement(nexusPanels, {section: "tools"}) }] : []),
      { id: "overview", label: t("modern", "overview"), content: overviewTiles },
      { id: "performance", label: t("modern", "performance"), content: <GamePerformancePanel key={nexusGame?.id} value={nexusGame?.advanced} disabled={nexus.busy || nexus.settingsBusy || Boolean(nexusGame?.running)} onChange={value => nexusGame ? nexus.configureAdvanced(nexusGame.id, value) : Promise.resolve(false)} /> },
      { id: "options", label: t("modern", "options"), content: cloneElement(nexusPanels, {section: "options"}) },
      { id: "advanced", label: language === "pt" ? "Avançado" : "Advanced", content: cloneElement(nexusPanels, {section: "advanced"}) },
    ]
    : [
      { id: "overview", label: t("modern", "overview"), content: overviewTiles },
      ...(installedModCount > 0 ? [{ id: "nexus", label: t("modern", "mods"), count: installedModCount, content: integrationPanel("mods") }] : []),
      ...(dlssNrInstalled ? [{ id: "dlss", label: t("modern", "dlssConfig"), content: integrationPanel("dlss") }] : []),
      { id: "performance", label: t("modern", "performance"), content: executionPanel("performance") },
      { id: "options", label: t("modern", "options"), content: executionPanel("options") },
      { id: "advanced", label: language === "pt" ? "Avançado" : "Advanced", content: executionPanel("advanced") },
    ];
  const gameGrid = <GameGrid key={(gamePanelMode === "nexus" || addingNexusGame) ? "nexus" : "launcher"} managedLibrary={(gamePanelMode === "nexus" || addingNexusGame) ? { games: nexus.visibleGames.map(item => ({ ...item.game, nexus_platform: item.platform, nexus_id: item.id, nexus_running: item.running, nexus_starting: nexus.launching === item.id && nexus.stopping !== item.id })), busy: nexus.busy, loaded: nexus.loaded, scanning: nexus.scanning, error: nexus.error, title: nexusText.title, downloadsAction: null, accountLabel: nexusText.account, connectLabel: nexusAccountConnected === false ? nexusText.connectLibraryAccount : nexusText.account, onAccount: () => { setSettingsSection("nexusAccount"); setSettingsPage("connections"); setShowSettingsModal(true); }, empty: nexusText.empty, hint: nexusText.emptyHint, addLabel: nexusText.add, onAdd: openNexusAddition, onRefresh: () => { void nexus.refresh(); }, onCoverFallback: nexus.recoverCover } : undefined} onCoverAction={coverAction} coverHoldBlocked={gameInstaller.busy || libraryMutationBusy || nexus.busy || Boolean(gameExecution.pendingPath)} coverActionsBlocked={gameInstaller.busy || libraryMutationBusy || nexus.busy || gameExecution.active || Boolean(gameExecution.pendingPath)} homeRevision={homeRevision} onSelectGame={handleSelectGame} startingGamePath={gameExecution.pendingPath || (gameExecution.session?.state === "starting" ? gameExecution.session.path : undefined)} runningGamePath={gameExecution.session?.state === "running" ? gameExecution.session.path : undefined} selectedGameKey={selectedGame ? gameViewKey(selectedGame) : undefined} mode={gamePanelMode} libraryScope={libraryScope} modeDisabled={gameInstaller.busy || libraryMutationBusy || nexus.busy} onModeChange={changeGamePanelMode} onCountChange={setLibraryCount} />;

  const selectedBanner = useGameBanner(selectedGame || undefined, !["install", "add"].includes(gamePanelMode));
  const installingGame = gamePanelMode === "install" || gamePanelMode === "add";
  const installationBanner = useGameBanner(installingGame && installationDraft.name && !installationDraft.editing ? { name: installationDraft.name, path: "installation-draft", launcher: "PeliGames", cover_url: installationDraft.coverStatus === "found" ? installationDraft.coverUrl : undefined } : undefined, true);
  const modernCatalogOpen = gamePanelMode === "nexus" && detailsOpen && Boolean(nexusGame) && nexusTab === "catalog";
  const modernDetails = <GameBenchmarkNavigationContext.Provider value={selectedGame ? openGameBenchmarks : null}><ModernGameDetails resetKey={detailsResetKey}
    contentView={benchmarkOpen && selectedGame ? <GameBenchmarkView key={detailsResetKey} game={selectedGame} onBack={() => setBenchmarkViewKey(null)} /> : undefined}
    activeTab={detailsReturnTab?.key === detailsResetKey ? detailsReturnTab.tab : undefined}
    onTabChange={tab => setDetailsReturnTab({ key: detailsResetKey, tab })}
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
      {installedModCount > 0 && <span className="modern-meta-chip">{installedModCount} mods</span>}
    </> : undefined}
    actions={modernActions ?? undefined}
    requestedTab={gamePanelMode === "nexus" && nexusModToView && nexusModToView.gameId === nexusGame?.id ? "nexus" : undefined}
    catalog={modernCatalogOpen ? <NexusCatalogPanel key={nexusGame!.id} embedded game={nexusGame} onAssociate={nexus.configureDomain} onImport={nexus.importArchive}
      onClose={() => setNexusTab("mods")} onLibrary={() => { setNexusTab("mods"); collapseGameDetails(); }} /> : undefined}
    tabs={modernTabs} onClose={collapseGameDetails} closeLabel={t("modern", "closeDetails")} /></GameBenchmarkNavigationContext.Provider>;
  const modernMain = <ModernShell active={modernSection} disabled={hasOpenDialog || (showSettingsModal && preferencesBusy)} onNavigate={navigateModern} activeDownloads={activeDownloads}
    libraryCount={libraryCount} settingsPage={modernSettingsPage}
    onSettingsNavigate={next => { if (!preferencesBusy) setSettingsPage(next); }}>
    {showSettingsModal && <div className="modern-scroll modern-settings-scroll">
      <SettingsModal page={settingsPage} onPageChange={setSettingsPage} onBusyChange={setPreferencesBusy} initialSection={settingsSection} initialBackendArch={settingsSection === "backendUpdate" ? notificationBackendArch : undefined}
        onClose={() => setShowSettingsModal(false)}
        onConfigUpdated={(config) => { setAppConfig(config); if (!config) setShowSetupWizard(true); }}
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
          notificationIndicator={<NotificationCenter onNexusAccount={() => { setShowSettings(false); setSettingsSection("nexusAccount"); setSettingsPage("connections"); setShowSettingsModal(true); }} nexusError={nexus.error} navigationBusy={preferencesBusy || Boolean(launcherDownloadProgress)} launcherUpdate={launcherUpdateInfo} onNavigate={target => {
            setShowSettings(false);
            if (target === "launcher") setShowAppUpdate(true);
            else if (target === "ge-proton" || target === "cachyos-proton") { setSettingsSection("protons"); setShowSettingsModal(true); }
            else { setNotificationBackendArch(target); setSettingsSection("backendUpdate"); setShowSettingsModal(true); }
          }}/>}
          updateIndicator={<LauncherUpdateIndicator status={launcherUpdateStatus} disabled={hasOpenDialog} downloadProgress={launcherDownloadProgress}
            onCheck={checkUpdatesManually}
            onOpen={() => { setShowSettings(false); setShowAppUpdate(true); }} />}
          settingsMenu={
            <div style={{ position: "relative" }} ref={settingsRef}>
              <button 
                data-tauri-drag-region="false"
                aria-pressed={showSettings || showSettingsModal}
                aria-haspopup="dialog" aria-expanded={showSettings} aria-controls="launcher-settings-menu"
                className="btn-titlebar" 
                style={{ padding: "0.3rem 0.5rem", borderRadius: "4px", display: "flex", alignItems: "center", justifyContent: "center", background: showSettings ? "rgba(var(--surface-255-255-255, 255, 255, 255), 0.15)" : "transparent", border: "none", color: "var(--tone-94a3b8, #94a3b8)", cursor: "pointer" }} 
                disabled={hasOpenDialog}
                onClick={() => { if (!hasOpenDialog) setShowSettings(!showSettings); }}
                title={t("settings", "title")}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
              </button>

              {showSettings && (
                <div id="launcher-settings-menu" className="dialog-popover launcher-settings-menu" role="dialog" aria-label={t("settings", "title")}>
                  <div className="launcher-settings-items">
                    <button 
                      className="launcher-settings-preferences"
                      onClick={() => { setSettingsSection("general"); setShowSettingsModal(true); setShowSettings(false); }}
                    >
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
                        <span>{t("settings", "preferences")}</span>
                      </div>
                    </button>
                    <div className="effects-preference">
                      <span className="effects-preference-label">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20"/></svg>
                        {t("settings", "language")}
                      </span>
                      <div className="effects-menu-anchor">
                        <button type="button" className="effects-menu-trigger" aria-label={t("settings", "language")}
                          aria-haspopup="menu" aria-expanded={showLangDropdown}
                          onClick={e => { e.stopPropagation(); setShowLangDropdown(!showLangDropdown); setShowEffectsDropdown(false); }}>
                          {language === "pt" ? "BR" : "EN"}
                          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="m6 9 6 6 6-6"/></svg>
                        </button>
                        {showLangDropdown && <div className="effects-menu" role="menu" onKeyDown={e => { if (e.key === "Escape") setShowLangDropdown(false); }}>
                          {(["pt", "en"] as const).map(lang => <button key={lang} type="button" role="menuitemradio" aria-checked={language === lang}
                            onClick={() => { setLanguage(lang); setShowLangDropdown(false); setShowSettings(false); }}>
                            {lang === "pt" ? "BR" : "EN"}
                          </button>)}
                        </div>}
                      </div>
                    </div>

                    <div className="effects-preference">
                      <span className="effects-preference-label"><MenuIcon name="bolt" /> {t("settings", "effects")}</span>
                      <div className="effects-menu-anchor">
                        <button type="button" className="effects-menu-trigger" aria-label={t("settings", "effects")}
                          aria-haspopup="menu" aria-expanded={showEffectsDropdown} aria-controls="interface-effects-menu"
                          onClick={() => { setShowEffectsDropdown(!showEffectsDropdown); setShowLangDropdown(false); }}
                          onKeyDown={e => { if (e.key === "Escape") setShowEffectsDropdown(false); }}>
                          {t("settings", performanceMode ? "performance" : "elegant")}
                          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="m6 9 6 6 6-6"/></svg>
                        </button>
                        {showEffectsDropdown && <div id="interface-effects-menu" className="effects-menu" role="menu"
                          onKeyDown={e => { if (e.key === "Escape") setShowEffectsDropdown(false); }}>
                          <button type="button" role="menuitemradio" aria-checked={!performanceMode} onClick={() => { setPerformanceMode(false); setShowEffectsDropdown(false); }}>{t("settings", "elegant")}</button>
                          <button type="button" role="menuitemradio" aria-checked={performanceMode} onClick={() => { setPerformanceMode(true); setShowEffectsDropdown(false); }}>{t("settings", "performance")}</button>
                        </div>}
                      </div>
                    </div>
                    <div className="launcher-settings-scale">
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><line x1="11" x2="11" y1="8" y2="14"/><line x1="8" x2="14" y1="11" y2="11"/></svg>
                        <span>{t("settings", "scale")}</span>
                      </div>
                      <div className="launcher-settings-scale-controls">
                        <button className="launcher-settings-scale-button" onClick={() => setUiScale(Math.max(0.6, uiScale - 0.1))}>-</button>
                        <span style={{ fontSize: "0.9rem", color: "var(--accent-light)" }}>{Math.round(uiScale * 100)}%</span>
                        <button className="launcher-settings-scale-button" disabled={uiScale >= MAX_INTERFACE_SCALE} onClick={() => setUiScale(Math.min(MAX_INTERFACE_SCALE, Math.round((uiScale + 0.1) * 10) / 10))}>+</button>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </div>
          } 
        />
        {modernMain}
      <WindowResizeHandles />
      <div className="launcher-build-label">{APP_BUILD_LABEL}</div>
    </div>

    {showConfirmGameUninstall && (
      <ConfirmModal 
        title={t("confirm", "attention")}
        message={t("confirm", "uninstallGame")}
        isDanger={true}
        confirmText={t("confirm", "uninstallBtn")}
        cancelText={t("confirm", "cancel")}
        onCancel={() => setShowConfirmGameUninstall(null)}
        onConfirm={() => {
          const path = showConfirmGameUninstall;
          setShowConfirmGameUninstall(null);
          proceedUninstallForPath(path);
        }}
      />
    )}


    {showAppUpdate && <AppUpdateModal onDownloadProgress={setLauncherDownloadProgress} initialInfo={launcherUpdateInfo ?? undefined} onClose={() => setShowAppUpdate(false)} />}

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
    {executableChoice && <ExecutableChoiceModal entries={executableChoice.entries} prefix={executableChoice.prefix} changing={executableChoice.changing} currentPath={selectedGame?.path} busy={libraryMutationBusy} error={executableChoiceError}
      onKeepAll={() => { const first = executableChoice.entries[0]; setExecutableChoice(null); if (first && !executableChoice.changing) showCompletedGameEntry(first); }}
      onConfirm={path => void (async () => {
        if (libraryMutationLock.current) return;
        libraryMutationLock.current = true; setLibraryMutationBusy(true); setExecutableChoiceError("");
        try {
          const game = await choosePeliGamesEntry(path);
          setExecutableChoice(null);
          if (executableChoice.changing) {
            setSelectedGame(previous => previous ? { ...game, library_view: previous.library_view } : previous);
            setGameDir(game.directory || game.path);
          } else showCompletedGameEntry(game);
          window.dispatchEvent(new Event("peligamesLibraryChanged"));
        } catch (error) { setExecutableChoiceError(String(error)); }
        finally { libraryMutationLock.current = false; setLibraryMutationBusy(false); }
      })()} />}
    {launcherGames && <LauncherGamesModal games={launcherGames.games} busy={libraryMutationBusy} error={launcherGamesError}
      onLater={() => { if (!libraryMutationBusy) setLauncherGames(null); }}
      onAdd={(added, declined) => void (async () => {
        if (libraryMutationLock.current) return;
        libraryMutationLock.current = true; setLibraryMutationBusy(true); setLauncherGamesError("");
        try {
          await addLauncherGames(launcherGames.path, added, declined);
          setLauncherGames(null);
          window.dispatchEvent(new Event("peligamesLibraryChanged"));
        } catch (error) { setLauncherGamesError(String(error)); }
        finally { libraryMutationLock.current = false; setLibraryMutationBusy(false); }
      })()} />}
    {uninstallTarget && <PrefixUninstallModal name={uninstallTarget.name} prefix={uninstallTarget.prefix} busy={libraryMutationBusy} error={libraryMutationError}
      onCancel={() => { if (!libraryMutationBusy) { setUninstallTarget(null); setLibraryMutationError(""); } }} onConfirm={options => void confirmPrefixUninstall(options)} />}

    {appImageTarget && !showWelcome && !showAppUpdate && <AppImageInstallPrompt target={appImageTarget} onClose={() => setAppImageTarget("")} />}
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
      setSettingsSection(destination); setShowSettingsModal(true);
    }} />}

    {showSetupWizard && (
      <SetupWizard 
        allowCancel
        onCancel={closeDlssSetup}
        onComplete={(config) => {
          setAppConfig(previous => ({
            ...config,
            game_shortcut_keys: { ...previous?.game_shortcut_keys, ...config.game_shortcut_keys },
          }));
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


    {showModal && (
      <ResultModal 
        title={modalTitle} 
        message={modalMessage} 
        type={modalType} 
        logs={logs}
        onOpenSetup={modalType === "error" && modelRecovery ? () => { setShowModal(false); setModelRecovery(false); setShowSetupWizard(true); } : undefined}
        onClose={() => setShowModal(false)} 
      />
    )}

    {loading && loadingMessage && (
      <LoadingModal message={loadingMessage} />
    )}

    {gameLogsTarget && <GameLogsModal game={gameLogsTarget} onClose={() => setGameLogsTarget(null)} />}
    {coverContextMenu && selectedGame && <GameCoverMenu ref={coverContextMenuRef}
      game={{ ...selectedGame, library_view: gamePanelMode === "nexus" ? "nexus" : libraryScope }}
      x={coverContextMenu.x} y={coverContextMenu.y} onClose={() => setCoverContextMenu(null)}
      blocked={gameInstaller.busy || libraryMutationBusy || nexus.busy || gameExecution.active || Boolean(gameExecution.pendingPath)} onAction={coverAction} />}
    {removeLibraryTarget && <NexusPopup title="Remover da biblioteca" onClose={() => setRemoveLibraryTarget(null)} footer={<>
      <button className="btn btn-secondary" onClick={() => setRemoveLibraryTarget(null)}>Cancelar</button>
      <button className="btn btn-primary" disabled={nexus.busy} onClick={() => {
        const target = removeLibraryTarget;
        // The Nexus library entry is deleted for real; other libraries only hide the card.
        if (target.library_view === "nexus" && target.nexus_id) {
          void nexus.removeGame(target.nexus_id).then(removed => {
            if (!removed) return;
            if (selectedGame?.nexus_id === target.nexus_id) collapseGameDetailsNow();
            setRemoveLibraryTarget(null);
          });
          return;
        }
        setLibraryGameRemoved(target, true);
        if (selectedGame?.path === target.path) collapseGameDetailsNow();
        setRemoveLibraryTarget(null);
      }}><MenuIcon name="trash" />Remover</button>
    </>}>{removeLibraryTarget.library_view === "nexus"
      ? <><p>Remover {removeLibraryTarget.name} da biblioteca Nexus? O jogo e os pacotes de mods no disco serão preservados. Um jogo com mods instalados precisa ter os mods removidos antes.</p>{nexus.error && <p role="alert" className="nexus-error">{nexus.error}</p>}</>
      : <p>Remover {removeLibraryTarget.name} desta biblioteca? O jogo e os mods no disco serão preservados, assim como as entradas nas outras bibliotecas.</p>}</NexusPopup>}
    <NexusDownloadsPanel games={nexus.workspace.games} hideButton openRequest={downloadsRequest} onActiveCount={setActiveDownloads} onViewMod={viewDownloadedMod} />
    <NexusBrowserPopup />
    </EffectsContext.Provider>
  );
}

export default App;

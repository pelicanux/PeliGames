import { CoverGameActions, type CoverGameAction } from "./components/CoverGameActions";
import { GameLogsModal } from "./components/GameLogsModal";
import { gameViewKey } from "./services/gameIdentity";
import { GameEntryModal } from "./components/GameEntryModal";
import { addPeliGamesEntry, uninstallPeliGamesEntry, listPeliGamesEntries } from "./services/installedLibrary";
import { useGameExecution } from "./hooks/useGameExecution";
import { GameLaunchButton } from "./components/GameLaunchButton";
import { InstalledGamePanels } from "./components/InstalledGamePanels";
import { useInstallationLocation } from "./hooks/useInstallationLocation";
import { useGameInstaller } from "./hooks/useGameInstaller";
import { useInstallationGameDraft } from "./hooks/useInstallationGameDraft";
import { useNeuralStartup } from "./hooks/useNeuralStartup";
import { isInstalledMod } from "./services/installationStatus";
import { CoverContextMenuPanel, SteamGridCoverHint } from "./components/CoverContextMenu";
import { changeLocalCover, resetGameCover, reportCoverError } from "./services/customCovers";
import { useState, useEffect, useLayoutEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAnimatedDetailsHeight } from "./hooks/useAnimatedDetailsHeight";
import { useCoverFlight } from "./hooks/useCoverFlight";
import { useGameAnalysis } from "./hooks/useGameAnalysis";
import { useI18n } from "./i18n/I18nContext";
import { AnimatePresence, motion } from "framer-motion";
import "./App.css";
import "./theme/appearance.css";
import "./theme/glassSurfaces.css";
import { MenuIcon } from "./components/MenuIcon";
import type { GamePanelMode } from "./components/GameModeSelector";
import { GamePanelCarousel } from "./components/GamePanelCarousel";
import { GameInfoPanel } from "./components/GameInfoPanel";
import { InstallGameSummary, InstallGamePanels } from "./components/InstallGameView";

// Types
import { InstallRoute } from "./types/installer";

// Services
import { isMissingModel, localizeInstallationMessage } from "./services/installationMessages";
import { runInstallation } from "./services/installerService";
import { openDirectoryPicker } from "./services/tauriService";

// Components
import { RouteSelector } from "./components/RouteSelector";
import { BitnessSelector } from "./components/BitnessSelector";
import { InstallAction } from "./components/InstallAction";
import { ResultModal } from "./components/ResultModal";
import { UninstallModal } from "./components/UninstallModal";
import { LauncherUpdateIndicator } from "./components/LauncherUpdateIndicator";
import { checkLauncherUpdate, checkLauncherUpdateOnStartup, type LauncherDownloadProgress, type LauncherUpdateInfo, type LauncherUpdateStatus } from "./services/launcherUpdates";
import { AppUpdateModal } from "./components/AppUpdateModal";
import { BackendUpdaterModal } from "./components/BackendUpdaterModal";
import { LoadingModal } from "./components/LoadingModal";
import { CreditsModal } from "./components/CreditsModal";
import { TitleBar } from "./components/TitleBar";
import { SetupWizard, AppConfig } from "./components/SetupWizard";
import { SettingsModal } from "./components/SettingsModal";
import { ModManagerModal } from "./components/ModManagerModal";
import { WelcomeModal } from "./components/WelcomeModal";
import { hasDlssnrConfiguration } from "./services/modConfiguration";
import { GameGrid, GameInfo } from "./components/GameGrid";
import { InstructionsModal } from "./components/InstructionsModal";
import { ConfirmModal } from "./components/ConfirmModal";
import { ShortcutKeySelector } from "./components/ShortcutKeySelector";

import { AmbientBackground } from "./components/AmbientBackground";
import { EffectsContext } from "./components/EffectsContext";
import { APP_BUILD_LABEL } from "./services/buildInfo";
import { defaultShortcutForRoute, steamLaunchOptionsForRoute } from "./services/routeDefaults";

function App({ initialGamePath }: { initialGamePath?: string }) {
  const [performanceMode, setPerformanceMode] = useState(() => localStorage.getItem("performance_mode") === "true");
  const coverFlight = useCoverFlight(performanceMode);
  useEffect(() => {
    if (!initialGamePath) return;
    let active = true;
    listPeliGamesEntries().then(entries => {
      const entry = entries.find(game => game.path === initialGamePath);
      if (active && entry) { setSelectedGame(entry); setGamePanelMode("installed"); setLibraryScope("own"); setGameDir(entry.directory || entry.path); }
    }).catch(console.error);
    return () => { active = false; };
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
  const [bitness, setBitness] = useState<"32" | "64">("64");
  const [gpuArch, setGpuArch] = useState<"rdna4" | "rdna3">("rdna4");
  const { t, language, setLanguage } = useI18n();
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
  const [settingsSection, setSettingsSection] = useState<"general" | "covers" | "protons">("general");
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
  const [isEditingPath, setIsEditingPath] = useState(false);
  const [editPathValue, setEditPathValue] = useState("");
  const coverContextMenuRef = useRef<HTMLDivElement>(null);
  const [gameLogsTarget, setGameLogsTarget] = useState<GameInfo | null>(null);
  const [coverContextMenu, setCoverContextMenu] = useState<{ x: number, y: number } | null>(null);
  const settingsRef = useRef<HTMLDivElement>(null);
  const [uiScale, setUiScale] = useState(() => {
    const saved = localStorage.getItem("ui_scale");
    return saved ? parseFloat(saved) : 1;
  });

  useEffect(() => {
    localStorage.setItem("ui_scale", uiScale.toString());
    document.documentElement.style.fontSize = `${16 * uiScale}px`;
  }, [uiScale]);

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
      const result = await invoke<{ status: string; installed_dll: string | null }>("get_game_installation_details", { gameDir: path });
      if (request === installationRequest.current) { setInstallStatus(result.status); setInstalledDll(result.installed_dll); }
    } catch (error) {
      console.error("Could not refresh installation details:", error);
      if (request === installationRequest.current) { setInstallStatus("Erro"); setInstalledDll(null); }
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
        dllPath: appConfig.dll_version.endsWith('.dll') ? appConfig.dll_version : null,
        binPath: appConfig.dll_version.endsWith('.bin') ? appConfig.dll_version : null,
        shortcutKey: appConfig?.game_shortcut_keys?.[gameDir] || defaultShortcutForRoute(route),
        neuralStartup: neuralStartup.enabled
      });
      setLogs((prev) => prev + "\n" + localizeInstallationMessage(response, language));


      const isRepair = isInstalledMod(installStatus);
      setModalTitle(isRepair ? t("app", "repairSuccessTitle") : t("app", "installSuccessTitle"));
      setModalMessage(isRepair ? t("app", "repairSuccessMsg") : t("app", "installSuccessMsg").replace("{launchOptions}", steamLaunchOptionsForRoute(route)));
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
      if (selectedGame?.path === gameDir) {
        await refreshInstallationDetails(gameDir);
        setGameInfoRevision(value => value + 1);
      }
      setLoading(false);
      setLoadingMessage("");
    }
  };

  const [showUninstallPrompt, setShowUninstallPrompt] = useState(false);
  const [showConfirmGameUninstall, setShowConfirmGameUninstall] = useState<string | null>(null);

  const handleUninstallClick = () => {
    setShowUninstallPrompt(true);
  };

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

      setModalTitle(t("app", "uninstallSuccessTitle"));
      setModalMessage(t("app", "uninstallSuccessMsg") + path);
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
      if (selectedGame && selectedGame.path === path) {
        await refreshInstallationDetails(path);
        setGameInfoRevision(value => value + 1);
      }
    }
  };

  const proceedUninstall = async () => {
    setShowUninstallPrompt(false);
    const path = await openDirectoryPicker("Selecione a pasta do jogo");
    if (!path) return; // User cancelled
    await proceedUninstallForPath(path);
  };

  const [selectedGame, setSelectedGame] = useState<GameInfo | null>(null);
  const [modManagerOpen, setModManagerOpen] = useState(false);
  const [showModManagerModal, setShowModManagerModal] = useState(false);
  const [gamePanelMode, setGamePanelMode] = useState<GamePanelMode>("install");
  const [homeRevision, setHomeRevision] = useState(0);
  const [libraryScope, setLibraryScope] = useState<"own" | "all">("own");
  const [showGameEntryModal, setShowGameEntryModal] = useState(false);
  const [libraryMutationBusy, setLibraryMutationBusy] = useState(false);
  const libraryMutationLock = useRef(false);
  const [libraryMutationError, setLibraryMutationError] = useState("");
  const [uninstallTarget, setUninstallTarget] = useState<{ path: string; name: string; prefix: string } | null>(null);
  const [installationOpen, setInstallationOpen] = useState(false);
  const installationDraft = useInstallationGameDraft();
  const installationLocation = useInstallationLocation(installationDraft.name, installationOpen);
  const [installationExecutable, setInstallationExecutable] = useState("");
  const [installationProton, setInstallationProton] = useState("");
  const gameInstaller = useGameInstaller();
  const gameExecution = useGameExecution();
  const installationReady = Boolean(!gameExecution.active && !gameExecution.pendingPath && installationDraft.name && !installationDraft.editing && installationLocation.selected && installationLocation.directory.trim() && installationExecutable.trim() && installationProton);
  const detailsOpen = (gamePanelMode === "install" || gamePanelMode === "add") ? installationOpen : gamePanelMode === "installed" ? Boolean(selectedGame) : modManagerOpen;
  const detailsSize = useAnimatedDetailsHeight(detailsOpen ? ((gamePanelMode === "install" || gamePanelMode === "add") ? "installation-draft" : selectedGame?.path ?? "mod-manager-empty") : undefined, performanceMode);
  const [isCollapsingGame, setIsCollapsingGame] = useState(false);
  const [ambientCoverUrl, setAmbientCoverUrl] = useState<string>();
  useEffect(() => {
    // Finish moving the library before repainting the full-window blurred background.
    if (!isCollapsingGame) setAmbientCoverUrl((gamePanelMode !== "install" && gamePanelMode !== "add") ? selectedGame?.cover_url : undefined);
  }, [selectedGame?.cover_url, isCollapsingGame, gamePanelMode]);
  const [showDiscardDraft, setShowDiscardDraft] = useState(false);
  const pendingDraftExit = useRef<(() => void) | null>(null);
  const hasUnfinishedDraft = installationOpen && (gamePanelMode === "install" || gamePanelMode === "add") && !gameInstaller.result && Boolean(
    installationDraft.name.trim() || installationDraft.draftName.trim() || installationLocation.selected || installationExecutable.trim() || installationProton
  );
  const requestDraftExit = (action: () => void) => {
    if (gameInstaller.busy || libraryMutationBusy || pendingDraftExit.current) return;
    if (hasUnfinishedDraft) {
      pendingDraftExit.current = action;
      setShowDiscardDraft(true);
    } else action();
  };
  const collapseGameDetailsNow = () => {
    if (gameInstaller.busy || libraryMutationBusy) return;
    gameInstaller.reset();
    setIsCollapsingGame(true);
    setCoverContextMenu(null);
    setSelectedGame(null);
    setInstallationOpen(false);
    setModManagerOpen(false);
    installationDraft.reset();
    installationLocation.reset(); setInstallationExecutable(""); setInstallationProton("");
    setGameDir("");
  };
  const collapseGameDetails = () => requestDraftExit(collapseGameDetailsNow);
  const returnToLauncherHome = () => requestDraftExit(() => {
    collapseGameDetailsNow();
    setGamePanelMode("install");
    setLibraryScope("own");
    setShowSettings(false);
    setIsEditingPath(false);
    setHomeRevision(value => value + 1);
  });
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
  const openModLibrary = () => requestDraftExit(() => {
    collapseGameDetailsNow();
    setIsEditingPath(false);
    setGamePanelMode("mods");
    setLibraryScope("all");
  });
  const [selectedGameCoverError, setSelectedGameCoverError] = useState(false);
  useEffect(() => {
    const coverChanged = (event: Event) => {
      const game = (event as CustomEvent<GameInfo>).detail;
      setSelectedGame(previous => previous?.path === game.path ? { ...previous, cover_url: game.cover_url, automatic_cover_url: game.automatic_cover_url } : previous);
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
  }, [language]);
  const [releaseDate, setReleaseDate] = useState<string>("Desconhecido");
  const [gameInfoRevision, setGameInfoRevision] = useState(0);
  useEffect(() => {
    const refresh = () => setGameInfoRevision(revision => revision + 1);
    window.addEventListener("gameInfoCacheCleared", refresh);
    return () => window.removeEventListener("gameInfoCacheCleared", refresh);
  }, []);
  const selectedGameDirectory = selectedGame?.directory ?? selectedGame?.path;
  const analysis = useGameAnalysis(gamePanelMode === "mods" ? selectedGameDirectory : undefined, selectedGame?.name, selectedGame?.app_id || undefined, gameInfoRevision);
  const upscalerFamilies = [...new Set((analysis.upscalers ?? []).map(name => {
    const family = name.trim().match(/^(DLSS|FSR|XeSS)(?=$|[\s\d._-])/i)?.[1]?.toLowerCase();
    return family === "dlss" ? "DLSS" : family === "fsr" ? "FSR" : family === "xess" ? "XeSS" : name;
  }))];
  const manualRoutes = useRef(new Map<string, InstallRoute>());
  const detectedBitness = analysis.architecture === "32-bits" ? "32" : analysis.architecture === "64-bits" ? "64" : null;
  useEffect(() => {
    if (detectedBitness) setBitness(detectedBitness);
  }, [selectedGame?.path, detectedBitness, gameInfoRevision]);
  const [installStatus, setInstallStatus] = useState<string>("Verificando...");
  const recommendOptiscaler = detectedBitness === "64" && bitness === "64" && upscalerFamilies.some(name => ["DLSS", "FSR", "XeSS"].includes(name));
  useEffect(() => {
    if (selectedGame && recommendOptiscaler && !manualRoutes.current.has(selectedGame.path)) changeInstallRoute("optiscaler");
  }, [selectedGame?.path, recommendOptiscaler]);
  const neuralStartup = useNeuralStartup(gamePanelMode === "mods" ? selectedGameDirectory : undefined, gamePanelMode === "mods" && isInstalledMod(installStatus));
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
      setInstalledDll(null);
      if (gamePanelMode !== "mods") return () => { active = false; };
      setInstallStatus("Verificando...");
      invoke<{ status: string; installed_dll: string | null }>("get_game_installation_details", { gameDir: selectedGameDirectory })
        .then(result => {
          if (active && request === installationRequest.current) { setInstallStatus(result.status); setInstalledDll(result.installed_dll); }
        })
        .catch(() => { if (active && request === installationRequest.current) { setInstallStatus("Erro"); setInstalledDll(null); } });

    } else {
      setReleaseDate("Desconhecido");
      setInstallStatus("Nenhum");
      setInstalledDll(null);
    }
    return () => { active = false; };
  }, [selectedGame?.path, selectedGame?.app_id, gameInfoRevision, gamePanelMode]);

  const handleSelectGame = (game: GameInfo, portrait?: HTMLElement, forceOpen = false) => requestDraftExit(() => {
    if (gameInstaller.busy || libraryMutationBusy) return;
    if (!forceOpen && gamePanelMode !== "install" && gamePanelMode !== "add" && selectedGame && gameViewKey(game) === gameViewKey(selectedGame)) {
      collapseGameDetails();
    } else {
      coverFlight.start(portrait);
      if (installationOpen) collapseGameDetailsNow();
      setIsCollapsingGame(false);
      setPlayGameEntrance(true);
      setInstallStatus("Verificando...");
      setInstalledDll(null);
      ++installationRequest.current;
      const manualRoute = manualRoutes.current.get(game.path);
      if (libraryScope === "all" && manualRoute) changeInstallRoute(manualRoute);
      setModManagerOpen(true);
      setGamePanelMode(libraryScope === "all" || game.launcher !== "PeliGames" ? "mods" : "installed");
      setGameDir(game.directory ?? game.path);
      setSelectedGame(game);
    }
  });

  const coverAction = (action: CoverGameAction, game: GameInfo) => {
    setCoverContextMenu(null);
    if (action === "favorite") return;
    if (action === "details") { handleSelectGame(game, undefined, true); return; }
    if (action === "logs") { setGameLogsTarget(game); return; }
    if (gameInstaller.busy || libraryMutationBusy || gameExecution.active || gameExecution.pendingPath) return;
    if (action === "start") {
      if (game.launcher === "PeliGames") void gameExecution.start(game.path);
      else if (game.launcher === "Steam") void invoke("launch_game", {path: game.path, appId: game.app_id, launcher: game.launcher}).catch(reportCoverError);
    } else if (game.launcher === "PeliGames") setUninstallTarget({path: game.path, name: game.name, prefix: game.prefix || ""});
    else if (game.launcher === "Steam") void invoke("request_steam_uninstall", { appId: game.app_id }).catch(reportCoverError);
  };

  // Successful registration is not a user request to abandon the draft. In
  // particular, async installation callbacks can still capture pre-save state.
  const showCompletedGameEntry = (game: GameInfo) => {
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
        const game = entries.find(entry => entry.prefix === result.prefix);
        if (game) showCompletedGameEntry(game);
      }
    }
  };
  const confirmPrefixUninstall = async () => {
    if (!uninstallTarget || libraryMutationLock.current || gameInstaller.busy || gameExecution.active || gameExecution.pendingPath) return;
    libraryMutationLock.current = true; setLibraryMutationBusy(true); setLibraryMutationError("");
    try {
      await uninstallPeliGamesEntry(uninstallTarget.path);
      setUninstallTarget(null);
      setSelectedGame(null); setInstallationOpen(false); setModManagerOpen(false); setGameDir("");
      gameInstaller.reset(); installationDraft.reset(); installationLocation.reset(); setInstallationExecutable(""); setInstallationProton("");
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
    } catch (error) { setLibraryMutationError(String(error)); }
    finally { libraryMutationLock.current = false; setLibraryMutationBusy(false); }
  };

  const handleCoverMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const card = e.currentTarget;
    if (performanceMode) return;
    const rect = card.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const centerX = rect.width / 2;
    const centerY = rect.height / 2;
    const rotateX = ((y - centerY) / centerY) * -15;
    const rotateY = ((x - centerX) / centerX) * 15;
    card.style.transform = `perspective(1000px) rotateX(${rotateX}deg) rotateY(${rotateY}deg) scale3d(1.02, 1.02, 1.02)`;
    const glare = card.querySelector('.glare') as HTMLDivElement;
    if (glare) {
      glare.style.background = `radial-gradient(circle at ${x}px ${y}px, rgba(255, 255, 255, 0.3) 0%, transparent 60%)`;
      glare.style.opacity = "1";
    }
  };

  const handleCoverMouseLeave = (e: React.MouseEvent<HTMLDivElement>) => {
    const card = e.currentTarget;
    card.style.transform = `perspective(1000px) rotateX(0deg) rotateY(0deg) scale3d(1, 1, 1)`;
    const glare = card.querySelector('.glare') as HTMLDivElement;
    if (glare) {
      glare.style.opacity = "0";
    }
  };

  const handleSaveCustomPath = async () => {
    if (!selectedGame) return;
    try {
      await invoke("save_custom_game_path", { gameName: selectedGame.name, newPath: editPathValue });
      // Update the cached library without scanning the launchers again.
      window.dispatchEvent(new CustomEvent("refreshGames", { detail: { oldPath: selectedGame.path, newPath: editPathValue } }));
      setSelectedGame(prev => prev ? { ...prev, path: editPathValue } : null);
      setGameDir(editPathValue);
      setIsEditingPath(false);
      
    } catch (e) {
      console.error("Failed to save custom path:", e);
    }
  };

  const hasOpenDialog = showDiscardDraft || showGameEntryModal || Boolean(uninstallTarget) || Boolean(gameLogsTarget) || showWelcome || showModManagerModal || showAppUpdate || showSetupWizard || showSettingsModal || showCreditsModal || showInstructionsModal || showUpdaterModal || showModal || showUninstallPrompt || !!showConfirmGameUninstall || loading;

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
      if (hasOpenDialog) return;
      setSettingsSection("protons");
      setShowSettingsModal(true);
    };
    window.addEventListener("openCoverPreferences", openCoverSettings);
    window.addEventListener("openProtonPreferences", openProtonSettings);
    return () => {
      window.removeEventListener("openCoverPreferences", openCoverSettings);
      window.removeEventListener("openProtonPreferences", openProtonSettings);
    };
  }, [hasOpenDialog]);

  return (
    <EffectsContext.Provider value={performanceMode}>
      {!performanceMode && <AmbientBackground coverUrl={ambientCoverUrl} />}
      {coverFlight.overlay}
      <div className={`app-wrapper${coverFlight.flying ? " cover-flight-active" : ""}`} style={{ animation: effectsHaveChanged ? "none" : undefined }}>
        <TitleBar 
          onHome={returnToLauncherHome} homeDisabled={gameInstaller.busy || libraryMutationBusy}
          onShowCredits={() => setShowCreditsModal(true)} 
          disabled={hasOpenDialog}
          updateIndicator={<LauncherUpdateIndicator status={launcherUpdateStatus} disabled={hasOpenDialog} downloadProgress={launcherDownloadProgress}
            onCheck={checkUpdatesManually}
            onOpen={() => { setShowSettings(false); setShowAppUpdate(true); }} />}
          settingsMenu={
            <div style={{ position: "relative" }} ref={settingsRef}>
              <button 
                data-tauri-drag-region="false"
                className="btn-titlebar" 
                style={{ padding: "0.3rem 0.5rem", borderRadius: "4px", display: "flex", alignItems: "center", justifyContent: "center", background: showSettings ? "rgba(var(--surface-255-255-255, 255, 255, 255), 0.15)" : "transparent", border: "none", color: "var(--tone-94a3b8, #94a3b8)", cursor: "pointer" }} 
                disabled={hasOpenDialog}
                onClick={() => { if (!hasOpenDialog) setShowSettings(!showSettings); }}
                title={t("settings", "title")}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
              </button>
              
              {showSettings && (
                <div className="dialog-popover" style={{ position: "absolute", top: "100%", right: 0, marginTop: "0.5rem", background: "rgba(var(--surface-15-10-28, 15, 10, 28), 0.6)", backdropFilter: "blur(16px)", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)", borderRadius: "12px", boxShadow: "0 10px 25px rgba(0,0,0,0.5)", width: "220px", zIndex: 100, overflow: "hidden", display: "flex", flexDirection: "column", textAlign: "left" }}>
                  <div style={{ padding: "0.8rem 1rem", borderBottom: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)" }}>
                    <span style={{ fontSize: "0.85rem", color: "var(--tone-94a3b8, #94a3b8)", fontWeight: 600 }}>{t("settings", "title")}</span>
                  </div>
              
                  <div style={{ padding: "0.5rem", display: "flex", flexDirection: "column", gap: "0.2rem" }}>
                    <button 
                      className="btn btn-secondary" 
                      style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "0.5rem 0.8rem", width: "100%", borderRadius: "6px", background: "transparent", border: "none" }}
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
                    <div style={{ padding: "0.5rem 0.8rem", display: "flex", flexDirection: "column", gap: "0.5rem" }}>
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><line x1="11" x2="11" y1="8" y2="14"/><line x1="8" x2="14" y1="11" y2="11"/></svg>
                        <span>{t("settings", "scale")}</span>
                      </div>
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", justifyContent: "space-between" }}>
                        <button className="btn btn-secondary" style={{ padding: "0.2rem 0.5rem", borderRadius: "4px" }} onClick={() => setUiScale(Math.max(0.6, uiScale - 0.1))}>-</button>
                        <span style={{ fontSize: "0.9rem", color: "var(--accent-light)" }}>{Math.round(uiScale * 100)}%</span>
                        <button className="btn btn-secondary" style={{ padding: "0.2rem 0.5rem", borderRadius: "4px" }} onClick={() => setUiScale(Math.min(2.0, uiScale + 0.1))}>+</button>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </div>
          } 
        />
        <motion.div layoutScroll className="container" style={{ padding: '1rem', paddingTop: '0.5rem', height: 'calc(100vh - 50px)', overflow: 'auto', overflowAnchor: 'none' }}>

      <div style={{ display: "flex", flexDirection: "column", flex: 1, gap: 0, minHeight: 0 }}>
        
        {/* Top Section: Selected Game Details & Settings */}
        <AnimatePresence onExitComplete={() => setIsCollapsingGame(false)}>
        {detailsOpen && (
          <motion.section key="game-details" className="selected-game-shell"
            initial={performanceMode || !playGameEntrance ? false : { height: 0 }} animate={{ height: detailsSize.height ?? "auto" }} exit={{ height: 0, overflow: "hidden" }}
            style={{ overflow: detailsSize.resizing && !performanceMode ? "hidden" : "visible" }}
            onAnimationStart={detailsSize.startResize}
            onAnimationComplete={detailsSize.finishResize}
            transition={{ duration: performanceMode ? 0 : 0.42, ease: [0.4, 0, 0.2, 1] }}>
          <motion.div ref={detailsSize.contentRef} style={{ paddingBottom: 32 }} initial={performanceMode || !playGameEntrance ? false : { y: 180, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: performanceMode ? 0 : 180, opacity: 0 }} transition={{ duration: performanceMode ? 0 : 0.42, ease: [0.4, 0, 0.2, 1] }}>
          <div className="selected-game-menu">
            
            {/* Left: Cover & Info */}
            {(gamePanelMode === "install" || gamePanelMode === "add") ? <InstallGameSummary mode={gamePanelMode} draft={installationDraft} ready={installationReady} busy={gameInstaller.busy || libraryMutationBusy} installationResult={gameInstaller.result} installationError={libraryMutationError || gameInstaller.error}
              onInstall={() => void submitGameEntry()}
              onUninstall={gameInstaller.result ? () => setUninstallTarget({ path: gameInstaller.result!.prefix.replace(/[\\/]prefix[\\/]?$/, ""), prefix: gameInstaller.result!.prefix, name: installationDraft.name }) : undefined}
              onCollapse={collapseGameDetails} onCoverMove={handleCoverMouseMove} onCoverLeave={handleCoverMouseLeave} reducedMotion={performanceMode} /> : !selectedGame ? <InstallGameSummary draft={installationDraft} mode="mods" onCollapse={collapseGameDetails} onCoverMove={handleCoverMouseMove} onCoverLeave={handleCoverMouseLeave} reducedMotion={performanceMode} /> : <div className="game-summary">
              <div className="game-cover-column">
                <motion.div 
                  key={`${gameViewKey(selectedGame)}-${performanceMode ? "performance" : "elegant"}`}
                  className="selected-cover"
                  ref={coverFlight.destinationRef}
                  layout={performanceMode ? false : "preserve-aspect"}
                  layoutId={performanceMode || coverFlight.flying ? undefined : `cover-${gameViewKey(selectedGame)}`}
                  onMouseMove={handleCoverMouseMove}
                  onMouseLeave={handleCoverMouseLeave}
                  style={{ 
                    position: "relative",
                    borderRadius: "12px", 
                    overflow: "hidden",
                    transition: "box-shadow 0.1s ease-out",
                    boxShadow: "0 0 20px rgba(var(--accent-rgb), 0.6)",
                    border: "2px solid rgba(var(--accent-rgb), 0.8)",
                    transformStyle: "preserve-3d",
                    willChange: "transform",
                    cursor: "pointer",
                    height: "max-content",
                    display: "inline-block"
                  }}
                  onContextMenu={(e) => { e.preventDefault(); setCoverContextMenu({ x: e.clientX, y: e.clientY }); }}
                >
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
                      <img src="/peligames.png" alt="Icon" style={{ position: "absolute", top: "50%", left: "50%", transform: "translate(-50%, -50%)", width: "60px", height: "60px", opacity: 0.5 }} />
                    </div>
                  )}
                </motion.div>
                <button 
                  onClick={collapseGameDetails}
                  className="btn game-back-button"
                  style={{ width: "100%", marginTop: "0.5rem", padding: "0.4rem 0.6rem", fontSize: "0.85rem", borderRadius: "8px", background: "rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)", color: "var(--tone-e2e8f0, #e2e8f0)", display: "flex", alignItems: "center", justifyContent: "center", gap: "0.4rem", cursor: "pointer", transition: "all 0.2s" }}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="m6 9 6 6 6-6"/></svg>
                  {t("gameInfo", "backToGrid")}
                </button>
              </div>

              {/* Left Panel: Title & Stacked Buttons */}
              <div className="game-action-column">
                <h3 className="selected-game-title">{selectedGame.name}</h3>
                
                <div className="game-action-stack">
                  {selectedGame.launcher === "PeliGames" && <GameLaunchButton path={selectedGame.path} execution={gameExecution} disabled={gameInstaller.busy} />}
                  {selectedGame.launcher === "PeliGames" && gamePanelMode === "installed" && <button type="button" className="btn btn-secondary game-uninstall-button"
                    disabled={gameInstaller.busy || libraryMutationBusy || gameExecution.active || Boolean(gameExecution.pendingPath)}
                    onClick={() => setUninstallTarget({ path: selectedGame.path, name: selectedGame.name, prefix: selectedGame.prefix || "" })}><MenuIcon name="trash" />{t("gameInfo", "uninstall")}</button>}
                  {selectedGame.launcher === "Steam" && (
                    <button 
                      className="btn btn-play game-play-button" 
                      onClick={() => {
                        invoke("launch_game", { 
                          path: selectedGame.path, 
                          appId: selectedGame.app_id || null, 
                          launcher: selectedGame.launcher 
                        }).catch((e: any) => (setModalTitle(t("gameInfo", "startGame")), setModalMessage(String(e)), setModalType("error"), setShowModal(true)));
                      }}
                      style={{ 
                        background: "var(--tone-10b981, #10b981)", color: "white", border: "none", 
                        padding: "0.6rem", borderRadius: "8px", fontWeight: "bold", fontSize: "1rem",
                        cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "center", gap: "0.5rem",
                        boxShadow: "0 0 10px rgba(16, 185, 129, 0.4)", transition: "all 0.2s ease", width: "100%"
                      }}
                    >
                      <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z" /></svg> {t("gameInfo", "startGame")}
                    </button>
                  )}
                  
                  {gamePanelMode === "mods" && <div className="game-install-action" style={{ width: "100%" }}>
                    <InstallAction onInstall={handleInstall} onUninstall={handleUninstallClick} loading={loading} disabled={gpuArch === "rdna3" && bitness === "32"} temporarilyBlocked={neuralStartup.busy} installStatus={installStatus} />
                  </div>}
                  
                  {gamePanelMode === "mods" && isInstalledMod(installStatus) && (
                    <button 
                      className="btn btn-secondary game-uninstall-button" 
                      onClick={() => setShowConfirmGameUninstall(selectedGameDirectory!)}
                      style={{ 
                        background: "rgba(239, 68, 68, 0.1)", color: "var(--tone-ef4444, #ef4444)", border: "1px solid rgba(239, 68, 68, 0.2)", 
                        padding: "0.5rem", borderRadius: "8px", fontWeight: "bold", cursor: "pointer",
                        display: "flex", alignItems: "center", justifyContent: "center", gap: "0.5rem", width: "100%"
                      }}
                    >
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path><line x1="10" x2="10" y1="11" y2="17"></line><line x1="14" x2="14" y1="11" y2="17"></line></svg>
                      {t("gameInfo", "uninstall")}
                    </button>
                  )}
                  


                  <a className="game-folder-link"
                    onClick={() => {
                      invoke("open_folder", { path: selectedGameDirectory }).catch((error: unknown) => {
                        setModalTitle(t("gameInfo", "openFolder")); setModalMessage(String(error));
                        setModalType("error"); setShowModal(true);
                      });
                    }}
                    style={{ 
                      color: "var(--tone-94a3b8, #94a3b8)", fontSize: "0.9rem", textAlign: "center", cursor: "pointer", textDecoration: "underline",
                      marginTop: "0.25rem", display: "block"
                    }}
                  >
                    {t("gameInfo", "openFolder")}
                  </a>
                  
                  {selectedGame.launcher === "Manual" && (
                    <a className="game-remove-link"
                      onClick={() => {
                        const saved = localStorage.getItem("custom_folders");
                        if (saved) {
                          const folders = JSON.parse(saved);
                          const newFolders = folders.filter((f: string) => f !== selectedGame.path);
                          localStorage.setItem("custom_folders", JSON.stringify(newFolders));
                          window.dispatchEvent(new Event("refreshGames"));
                          setSelectedGame(null);
                          setGameDir("");
                        }
                      }}
                      style={{ 
                        color: "var(--tone-ef4444, #ef4444)", fontSize: "0.9rem", textAlign: "center", cursor: "pointer", textDecoration: "underline",
                        marginTop: "0.5rem", display: "block"
                      }}
                    >
                      {t("gameInfo", "removeGame")}
                    </a>
                  )}
                </div>
              </div>
            </div>

            }

            {/* Right Panel: Game Info & Selectors */}
            <GamePanelCarousel mode={gamePanelMode} reducedMotion={performanceMode} installed={<InstalledGamePanels key={selectedGame?.path} game={selectedGame} disabled={gameInstaller.busy || libraryMutationBusy || gameExecution.active || Boolean(gameExecution.pendingPath)}
              onSaved={game => { setSelectedGame(previous => previous?.path === game.path ? game : previous); }} onRunProgram={gameExecution.start} />}
              installation={<InstallGamePanels mode={gamePanelMode === "add" ? "add" : "install"} draft={installationDraft} directory={installationLocation.directory} directorySelected={installationLocation.selected} onDefaultDirectory={installationLocation.selectDefault} directoryError={installationLocation.error} busy={gameInstaller.busy || libraryMutationBusy} onDirectoryChange={installationLocation.setDirectory}
                executable={installationExecutable} onExecutableChange={setInstallationExecutable} proton={installationProton} onProtonChange={setInstallationProton} />}>
              <>
              {selectedGame ? <>
              
              {/* Game Info Column */}
              <GameInfoPanel directoryRow={
                  <div className="game-info-directory-row">
                  <span className="game-info-label"><MenuIcon name="folder" />{t("gameInfo", "directory")}</span>
                  <div className="game-directory-value">
                    {isEditingPath ? (
                      <>
                        <input 
                          type="text" 
                          value={editPathValue} 
                          onChange={e => setEditPathValue(e.target.value)} 
                          style={{ flex: 1, padding: "0.2rem 0.4rem", background: "rgba(0,0,0,0.5)", color: "var(--tone-ffffff, #fff)", border: "1px solid #475569", borderRadius: "4px", fontSize: "0.8rem", width: "100%" }}
                        />
                        <button onClick={handleSaveCustomPath} style={{ background: "transparent", border: "none", cursor: "pointer", color: "var(--tone-10b981, #10b981)", padding: 0 }} title={t("settings", "save")}>
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>
                        </button>
                        <button onClick={() => setIsEditingPath(false)} style={{ background: "transparent", border: "none", cursor: "pointer", color: "var(--tone-ef4444, #ef4444)", padding: 0 }} title={t("settings", "cancel")}>
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                        </button>
                      </>
                    ) : (
                      <>
                        <span className="game-path-text" title={selectedGameDirectory}>{selectedGameDirectory}</span>
                        <button 
                          onClick={() => {
                            setEditPathValue(selectedGameDirectory ?? selectedGame.path);
                            setIsEditingPath(true);
                          }} 
                          style={{ background: "transparent", border: "none", cursor: "pointer", color: "var(--tone-60a5fa, #60a5fa)", padding: 0, opacity: 0.7 }} 
                          disabled={selectedGame.launcher === "PeliGames"}
                          title={t("gameInfo", "editDirectory")}
                        >
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z"></path></svg>
                        </button>
                      </>
                    )}
                  </div>
                  </div>
              } release={releaseDate === "Desconhecido" ? t("gameInfo", "unknown") : (releaseDate === "Detectando..." ? "..." : releaseDate)}
                platform={localizeAnalysisValue(analysis.platform)}>
                  <div className="game-info-item">
                    <span className="game-info-label"><MenuIcon name="chip" />{t("gameInfo", "architecture")}</span>
                    <span className="info-value-pill info-architecture">{localizeAnalysisValue(analysis.architecture)}</span>
                  </div>
                  <div className="game-info-item">
                    <span className="game-info-label"><MenuIcon name="graphics" />{t("gameInfo", "graphicsApi")}</span>
                    <span className="info-value-pill info-api">{localizeAnalysisValue(analysis.graphics_api)}</span>
                  </div>
                  <div className="game-info-item">
                    <span className="game-info-label"><MenuIcon name="puzzle" />{t("gameInfo", "modInstalled")}</span>
                    <span className="info-value-pill info-mod-status" style={{ color: !isInstalledMod(installStatus) ? "var(--tone-ef4444, #ef4444)" : "var(--tone-10b981, #10b981)", background: !isInstalledMod(installStatus) ? "rgba(239,68,68,0.1)" : "rgba(16,185,129,0.1)", fontWeight: "bold" }}>
                      {installStatus === "Verificando..." ? t("gameInfo", "verifying") : (!isInstalledMod(installStatus) ? t("gameInfo", "no") : `${t("gameInfo", "yes")} (${installStatus.replace(/^Instalado\s*\((.*)\)$/, "$1")})`)}
                    </span>
                  </div>
                  <div className="game-info-item" title={t("upscalerInfo", "evidence")}>
                    <span className="game-info-label"><MenuIcon name="graphics" />{t("upscalerInfo", "title")}</span>
                    <span className={`info-value-pill info-upscalers ${upscalerFamilies.length > 0 ? "detected" : ""}`}>
                      {analysis.architecture === "Verificando..." ? t("gameInfo", "verifying") : upscalerFamilies.length ? upscalerFamilies.join(" · ") : t("upscalerInfo", "notFound")}
                    </span>
                  </div>
                  {isInstalledMod(installStatus) && (
                    <>
                      <motion.div initial={performanceMode ? false : { opacity: 0, y: 4 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: performanceMode ? 0 : 0.2 }} className="game-info-item">
                        <span className="game-info-label"><MenuIcon name="document" />{t("gameInfo", "injectedDll")}</span>
                        <span className="info-value-pill info-injected-dll" style={{ fontFamily: "monospace" }}>{installedDll || t("gameInfo", "unknown")}</span>
                      </motion.div>
                      <motion.div initial={performanceMode ? false : { opacity: 0, y: 4 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: performanceMode ? 0 : 0.2 }} className="game-info-item">
                        <span className="game-info-label"><MenuIcon name="logs" />{t("gameInfo", "gameLogs")}</span>
                        <a className="info-value-pill info-game-logs" onClick={() => {
                          invoke("collect_and_open_logs", { gameName: selectedGame.name, gameDir: selectedGameDirectory }).catch((error: unknown) => console.error("Error opening logs:", error));
                        }}>{t("gameInfo", "openLogs")}</a>
                      </motion.div>
                    </>
                  )}
              </GameInfoPanel>

              </> : <GameInfoPanel directoryRow={<p className="game-info-label" style={{ gridColumn: "1 / -1" }}>{t("gameModes", "selectGameHint")}</p>} release="—" platform="—" />}

              {/* Mod Configuration Column */}
              <div className="mod-config-panel menu-glass-panel">
                <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                  <MenuIcon name="settings" />
                  <span style={{ color: "var(--tone-ffffff, #fff)", fontWeight: "bold", fontSize: "0.95rem" }}>{t("gameInfo", "modConfig")}</span>
                </div>
                {/* BitnessSelector */}
                <div className="mod-control-group mod-bitness-group" style={{ display: "flex", flexDirection: "column", gap: "0.4rem" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                    <MenuIcon name="chip" />
                    <span style={{ fontSize: "0.85rem", color: "var(--tone-94a3b8, #94a3b8)" }}>{t("bitness", "title")}</span>
                  </div>
                  <BitnessSelector bitness={bitness} setBitness={setBitness} gpuArch={gpuArch} recommended={detectedBitness} />
                </div>

                {/* RouteSelector */}
                <div className="mod-control-group mod-route-group" style={{ display: "flex", flexDirection: "column", gap: "0.4rem", marginTop: "0.5rem" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                    <MenuIcon name="cube" />
                    <span style={{ fontSize: "0.85rem", color: "var(--tone-94a3b8, #94a3b8)" }}>{t("routeSelector", "title")}</span>
                  </div>
                  <RouteSelector route={route} recommended={recommendOptiscaler} setRoute={value => {
                    if (selectedGame) manualRoutes.current.set(selectedGame.path, value);
                    changeInstallRoute(value);
                  }} bitness={bitness} />
                </div>

                <div className="mod-startup-controls">
                {/* ShortcutKeySelector */}
                <div className="mod-control-group mod-shortcut-group" style={{ display: "flex", flexDirection: "column", gap: "0.4rem", marginTop: "0.5rem", opacity: !isInstalledMod(installStatus) ? 0.4 : 1, pointerEvents: !isInstalledMod(installStatus) ? "none" : "auto" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                    <MenuIcon name="keyboard" />
                    <span style={{ fontSize: "0.85rem", color: "var(--tone-94a3b8, #94a3b8)" }}>{t("gameInfo", "menuKey")}</span>
                  </div>
                  <ShortcutKeySelector 
                    route={route}
                    shortcutKey={appConfig?.game_shortcut_keys?.[gameDir] || defaultShortcutForRoute(route)}
                    setShortcutKey={(val) => {
                      if (appConfig && selectedGame && isInstalledMod(installStatus)) {
                        const newConfig = {
                          ...appConfig,
                          game_shortcut_keys: { ...appConfig.game_shortcut_keys, [selectedGame.path]: val },
                        };
                        setAppConfig(newConfig);
                        invoke("save_app_config", { config: newConfig }).catch(err => console.error("Failed to save config:", err));
                        invoke("update_shortcut_key_in_game", { gameDir: selectedGameDirectory, shortcutKey: val })
                          .catch(err => console.error("Failed to update shortcut in game:", err));
                      }
                    }}
                  />
                </div>
                <div className="mod-neural-startup" style={{ opacity: !isInstalledMod(installStatus) ? 0.4 : 1 }} title={t("neuralStartup", "hint")}>
                  <span className="mod-neural-label"><MenuIcon name="neural" />{t("neuralStartup", "title")}</span>
                  <button type="button" role="switch" aria-checked={neuralStartup.enabled} aria-label={t("neuralStartup", "title")}
                    className={`neural-startup-switch ${neuralStartup.enabled ? "enabled" : ""}`} disabled={!isInstalledMod(installStatus) || loading || neuralStartup.initializing || installStatus === "Verificando..."}
                    onClick={() => void neuralStartup.change(!neuralStartup.enabled)}>
                    <span className="neural-switch-track" aria-hidden="true"><span /></span>
                    <span>{t("neuralStartup", neuralStartup.enabled ? "enabled" : "disabled")}</span>
                  </button>
                </div>
                </div>
                {neuralStartup.feedback && <p role="status" className="neural-startup-feedback">{t("neuralStartup", neuralStartup.feedback)}</p>}
                {neuralStartup.error && <p role="alert" className="neural-startup-error">{t("neuralStartup", "error")} {neuralStartup.error}</p>}
              </div>
              </>
            </GamePanelCarousel>
          </div>
          </motion.div>
          <div className="selected-game-divider" />
          </motion.section>
        )}
        </AnimatePresence>

        {/* Bottom Section: Always visible Game Grid */}
        <div className={`game-library-section ${isCollapsingGame ? "game-library-section--collapsing" : ""}`} style={{ flex: 1, minHeight: "300px", display: "flex", flexDirection: "column" }}>
          <GameGrid onCoverAction={coverAction} coverActionsBlocked={gameInstaller.busy || libraryMutationBusy || gameExecution.active || Boolean(gameExecution.pendingPath)} onHome={returnToLauncherHome} homeRevision={homeRevision} onSelectGame={handleSelectGame} selectedGameKey={selectedGame ? gameViewKey(selectedGame) : undefined} mode={gamePanelMode} libraryScope={libraryScope} detailsOpen={detailsOpen} modeDisabled={gameInstaller.busy || libraryMutationBusy} onModeChange={changeGamePanelMode} />
        </div>
      </div>

      <div style={{ textAlign: "right", marginTop: "1rem", fontSize: "0.75rem", opacity: 0.6, fontFamily: "monospace", padding: "0 1rem" }}>
        {APP_BUILD_LABEL}
      </div>
      </motion.div>
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

    {showUninstallPrompt && (
      <UninstallModal 
        onCancel={() => setShowUninstallPrompt(false)} 
        onPickDirectory={proceedUninstall} 
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
    {uninstallTarget && <ConfirmModal title={t("gameModes", "uninstallTitle")}
      message={`${t("gameModes", "uninstallPrefixWarning")}\n${uninstallTarget.name}\n${uninstallTarget.prefix}${libraryMutationError ? `\n${libraryMutationError}` : ""}`}
      isDanger busy={libraryMutationBusy} confirmText={t("gameInfo", "uninstall")} cancelText={t("confirm", "cancel")}
      onCancel={() => { if (!libraryMutationBusy) { setUninstallTarget(null); setLibraryMutationError(""); } }} onConfirm={() => void confirmPrefixUninstall()} />}

    {showWelcome && <WelcomeModal onStart={() => {
      localStorage.setItem("peligames_welcome_completed", "1");
      setShowWelcome(false);
      collapseGameDetails();
      setGamePanelMode("install");
      setLibraryScope("own");
    }} />}

    {showSetupWizard && (
      <SetupWizard 
        allowCancel
        onCancel={() => setShowSetupWizard(false)}
        onBack={() => { setShowSetupWizard(false); setShowModManagerModal(true); }}
        onComplete={(config) => {
          setAppConfig(previous => ({
            ...config,
            game_shortcut_keys: { ...previous?.game_shortcut_keys, ...config.game_shortcut_keys },
          }));
          setShowSetupWizard(false);
          openModLibrary();
        }} 
      />
    )}

    {showModManagerModal && <ModManagerModal disabled={!appConfigLoaded} onClose={() => setShowModManagerModal(false)} onSelect={() => {
      setShowModManagerModal(false);
      if (hasDlssnrConfiguration(appConfig)) openModLibrary();
      else setShowSetupWizard(true);
    }} />}

    {showSettingsModal && (
      <SettingsModal initialSection={settingsSection}
        onClose={() => setShowSettingsModal(false)}
        onConfigUpdated={(config) => {
          setAppConfig(config);
          if (!config) setShowSetupWizard(true);
        }}
        onOpenWizard={() => setShowSetupWizard(true)}
      />
    )}

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
    {coverContextMenu && selectedGame && (
      <CoverContextMenuPanel onClose={() => setCoverContextMenu(null)} ref={coverContextMenuRef} x={coverContextMenu.x} y={coverContextMenu.y}>
        <CoverGameActions game={selectedGame} blocked={gameInstaller.busy || libraryMutationBusy || gameExecution.active || Boolean(gameExecution.pendingPath)} onAction={coverAction} />
        <SteamGridCoverHint />
        <button
          onClick={async () => {
            const game = selectedGame;
            setCoverContextMenu(null);
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
          const game = selectedGame;
          setCoverContextMenu(null);
          try { await resetGameCover(game); } catch (error) { reportCoverError(error); }
        }} style={{ background: "transparent", border: "none", color: "var(--tone-e2e8f0, #e2e8f0)", padding: "0.5rem 1rem", textAlign: "left", cursor: "pointer", fontSize: "0.9rem" }}>
          <MenuIcon name="repair" />{t("gameGrid", "resetCover")}
        </button>

        <button
          onClick={() => {
            window.dispatchEvent(new Event("rescanGames"));
            setCoverContextMenu(null);
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
    </EffectsContext.Provider>
  );
}

export default App;

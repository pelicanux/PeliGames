import "./installerStyles";
import { useInstallerText } from "./installerText";
import { InstallerSettingsMenu } from "./InstallerSettingsMenu";
import { findPelinstallMatches, repairPeliGamesEntry, getPelinstallIcon, type ExecutableMatch } from "../../services/pelinstall";
import { fetchGameCoverByName } from "../../services/customCovers";
import { lazy, Suspense, useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { BrandName } from "../../components/BrandName";
import { InstallationActionIcon } from "../../components/InstallationActionIcon";
import { MenuIcon } from "../../components/MenuIcon";
import { PelinstallProtonPicker } from "./PelinstallProtonPicker";
import { HoverTooltip } from "../../components/HoverTooltip";
import { ConfirmModal } from "../../components/ConfirmModal";
import { useInstallationLocation } from "../../hooks/useInstallationLocation";
import { openDirectoryPicker, openFilePicker } from "../../services/tauriService";
import { runGameInstaller, type GameInstallationResult } from "../../services/gameInstallation";
import { addPeliGamesEntry, choosePeliGamesEntry, listPeliGamesEntries, uninstallPeliGamesEntry, type UninstallOptions } from "../../services/installedLibrary";
import { useI18n } from "../../i18n/I18nContext";
import { PrefixUninstallModal } from "../../components/PrefixUninstallModal";
import type { GameInfo } from "../../components/GameGrid";
import type { StartupInfo } from "../startup/StartupRouter";
// Proton settings load only when the user opens them.
const ProtonPreferencesDialog = lazy(() => import("../../components/ProtonPreferencesDialog").then(module => ({ default: module.ProtonPreferencesDialog })));
import { EntrySelector } from "./EntrySelector";
import { dragPelinstallWindow, PelinstallMinimizeButton } from "./WindowControls";

const preview = "__PELI_UI_PREVIEW__" in window;
/** Tallest the assistant grows to (the Proton list open); must match lib.rs. */
const PELINSTALL_MAX_HEIGHT = 720;
export function Pelinstall({ startup, embedded }: { startup: StartupInfo; embedded?: {
  mode: "install" | "add"; name: string; onClose: () => void; onBusyChange: (busy: boolean) => void;
} }) {
  const tr = useInstallerText();
  const { t } = useI18n();
  const completionPreview = !embedded && preview && new URLSearchParams(window.location.search).get("stage") === "complete";
  const sampleEntries: GameInfo[] = completionPreview ? ["7zFM", "7zG", "7z"].map(exe => ({ name: `7-Zip · ${exe}`, path: `/preview/${exe}.exe`, executable: `/preview/${exe}.exe`, launcher: "PeliGames", discovery_source: exe === "7zFM" ? "Atalho do Windows" : null })) : [];
  const panel = useRef<HTMLElement>(null);
  const [step, setStep] = useState(completionPreview ? 4 : embedded ? -2 : -1);
  const [mode, setMode] = useState<"install" | "add" | "repair" | null>(completionPreview ? "install" : embedded?.mode ?? null);
  const [file, setFile] = useState(startup.error ? "" : startup.executable || "");
  const [matches, setMatches] = useState<ExecutableMatch[]>([]);
  const [checking, setChecking] = useState(Boolean(!embedded && startup.executable && !startup.error));
  const [targetPath, setTargetPath] = useState("");
  const [repairExecutables, setRepairExecutables] = useState<GameInfo[]>([]);
  const [repairPrefix, setRepairPrefix] = useState("");
  const [uninstallConfirmation, setUninstallConfirmation] = useState(false);
  const [removed, setRemoved] = useState(false);
  const [executableIcon, setExecutableIcon] = useState("");
  const coverRequests = useRef(new Map<string, Promise<string | null>>());
  const target = matches.find(match => match.entry.path === targetPath) ?? matches[0];
  const [name, setName] = useState(completionPreview ? "7-Zip" : embedded?.name ?? "");
  const [proton, setProton] = useState("");
  const [protonPreferencesOpen, setProtonPreferencesOpen] = useState(false);
  const [protonPreferencesBusy, setProtonPreferencesBusy] = useState(false);
  const [desktop, setDesktop] = useState(completionPreview);
  const [existingShortcuts, setExistingShortcuts] = useState<string[]>([]);
  const [shortcutChecking, setShortcutChecking] = useState(false);
  const [shortcutCheckFailed, setShortcutCheckFailed] = useState(false);
  const [removeExistingShortcuts, setRemoveExistingShortcuts] = useState(false);
  const [updateExistingShortcuts, setUpdateExistingShortcuts] = useState(false);
  const [updatedShortcutCount, setUpdatedShortcutCount] = useState(0);
  const [removedShortcutCount, setRemovedShortcutCount] = useState(0);
  const [launcherOpening, setLauncherOpening] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(startup.error || "");
  const [result, setResult] = useState<GameInstallationResult | null>(completionPreview ? { prefix: "/preview/prefix", log: "/preview/prefix/logs/installation.log", registered_count: 3 } : null);
  const [entries, setEntries] = useState<GameInfo[]>(sampleEntries);
  const [chosen, setChosen] = useState("");
  // Discovery only recommends: even a single candidate requires confirmation.
  const [choosingExecutable, setChoosingExecutable] = useState(completionPreview);
  const [shortcut, setShortcut] = useState("");
  const [exitConfirmation, setExitConfirmation] = useState(false);
  const [cancelConfirmation, setCancelConfirmation] = useState(false);
  const [cancelPending, setCancelPending] = useState(false);
  const closeAfterCancel = useRef(false);
  const location = useInstallationLocation(name.trim(), mode !== null && Boolean(name.trim()) && step >= 0);
  const directory = mode === "repair" ? repairPrefix : location.directory;
  const directorySelected = mode === "repair" ? Boolean(repairPrefix.trim()) : location.selected;
  const canResolveDirectory = mode === "repair" ? directorySelected : location.hasSelection;
  const validFile = Boolean(file && (mode !== "add" || file.toLowerCase().endsWith(".exe")));
  const stateRef = useRef({ busy, dirty: false, installing: false });
  const closing = useRef(false);
  stateRef.current = { busy: busy || protonPreferencesBusy, dirty: !result && Boolean(directorySelected || proton || name.trim()), installing: busy && step === 3 };
  const close = async () => {
    if (embedded) { window.dispatchEvent(new Event("peligamesLibraryChanged")); embedded.onClose(); return; }
    if (preview) return;
    closing.current = true;
    try { await getCurrentWindow().destroy(); }
    catch (error) { closing.current = false; setError(String(error)); }
  };
  const requestClose = () => {
    if (stateRef.current.installing) { closeAfterCancel.current = true; setCancelConfirmation(true); return; }
    if (stateRef.current.busy) return;
    if (stateRef.current.dirty) setExitConfirmation(true);
    else void close();
  };
  useEffect(() => { embedded?.onBusyChange(busy || protonPreferencesBusy); }, [busy, protonPreferencesBusy, embedded?.onBusyChange]);
  const cancelInstallation = async () => {
    setCancelPending(true); setCancelConfirmation(false); setError("");
    try { await invoke("cancel_game_installer"); }
    catch (error) { closeAfterCancel.current = false; setCancelPending(false); setError(String(error)); }
  };
  useEffect(() => {
    if (preview || embedded) return;
    let disposed = false; let remove: (() => void) | undefined;
    getCurrentWindow().onCloseRequested(event => {
      if (closing.current) return;
      if (stateRef.current.busy || stateRef.current.dirty) { event.preventDefault(); requestClose(); }
    }).then(unlisten => { if (disposed) unlisten(); else remove = unlisten; }).catch(error => setError(String(error)));
    return () => { disposed = true; remove?.(); };
  }, []);
  useEffect(() => {
    if (embedded || !startup.executable || startup.error || (preview && startup.executable === "/preview/instalador.exe")) { setChecking(false); return; }
    let active = true;
    findPelinstallMatches(startup.executable).then(found => {
      if (active) { setMatches(found); setTargetPath(found[0]?.entry.path || ""); }
    }).catch(error => { if (active) setError(String(error)); }).finally(() => { if (active) setChecking(false); });
    return () => { active = false; };
  }, [startup.executable, startup.error]);
  const lookupShortcuts = async () => {
    if (!preview) return invoke<string[]>("find_peligames_shortcuts", { path: targetPath });
    const found = await findPelinstallMatches(target?.entry.executable || startup.executable || "");
    const shortcuts = found.find(match => match.entry.path === targetPath)?.shortcuts;
    if (!shortcuts) throw new Error("Não foi possível verificar os atalhos.");
    return shortcuts;
  };
  useEffect(() => {
    if (mode !== "repair" || step !== 2 || !targetPath) return;
    let active = true; let revision = 0;
    setDesktop(false); setRemoveExistingShortcuts(false); setUpdateExistingShortcuts(false);
    const refresh = () => {
      const current = ++revision;
      setShortcutChecking(true); setShortcutCheckFailed(false);
      lookupShortcuts().then(found => {
        if (active && current === revision) {
          setExistingShortcuts(found);
          if (found.length) setDesktop(false);
          else { setRemoveExistingShortcuts(false); setUpdateExistingShortcuts(false); }
        }
      }).catch(error => { if (active && current === revision) { setShortcutCheckFailed(true); setError(String(error)); } })
        .finally(() => { if (active && current === revision) setShortcutChecking(false); });
    };
    refresh(); window.addEventListener("focus", refresh);
    return () => { active = false; window.removeEventListener("focus", refresh); };
  }, [mode, step, targetPath]);
  useEffect(() => {
    if (mode !== "repair" || !target) return;
    let active = true;
    setRepairExecutables(matches.map(match => match.entry).filter(entry => entry.prefix === target.entry.prefix));
    listPeliGamesEntries().then(found => { if (active) setRepairExecutables(found.filter(entry => entry.prefix === target.entry.prefix)); })
      .catch(error => { if (active) setError(`Não foi possível consultar os executáveis: ${String(error)}`); });
    return () => { active = false; };
  }, [mode, targetPath]);
  const resolveCover = (title: string) => {
    const trimmed = title.trim();
    const query = target && trimmed === `${target.entry.name} (cópia)` ? target.entry.name : trimmed;
    if (target?.entry.name === query && target.entry.cover_url) return Promise.resolve(target.entry.cover_url);
    let request = coverRequests.current.get(query);
    if (!request) { request = fetchGameCoverByName(query).catch(() => null); coverRequests.current.set(query, request); }
    return request;
  };
  useEffect(() => {
    if (step !== 2 || !name.trim()) return;
    const timer = window.setTimeout(() => { void resolveCover(name); }, 600);
    return () => window.clearTimeout(timer);
  }, [name, step]);
  const iconSource = step === -1 && target ? target.entry.executable || startup.executable || "" : file;
  useEffect(() => {
    setExecutableIcon(""); if (!iconSource) return;
    let active = true; let url = "";
    getPelinstallIcon(iconSource).then(bytes => {
      if (active && bytes?.length) { url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "image/png" })); setExecutableIcon(url); }
    }).catch(() => {});
    return () => { active = false; if (url) URL.revokeObjectURL(url); };
  }, [iconSource]);
  const openSettings = async () => {
    if (!target || busy) return;
    if (preview) { window.open(`/preview.html?showGame=${encodeURIComponent(target.entry.path)}`, "_blank"); return; }
    setBusy(true); setError("");
    try { await invoke("open_peligames_launcher", { path: target.entry.path }); await close(); }
    catch (error) { setError(String(error)); } finally { setBusy(false); }
  };
  const beginExisting = (repair: boolean) => {
    if (!target) return;
    setError(""); location.reset(); setRepairPrefix(target.entry.prefix || "");
    if (repair) { setExistingShortcuts(target.shortcuts || []); setShortcutChecking(true); setDesktop(false); }
    setName(repair ? target.entry.name : `${target.entry.name} (cópia)`);
    setProton(target.entry.proton || ""); setTargetPath(target.entry.path);
    setMode(repair ? "repair" : target.installer ? "install" : "add");
    setFile(repair ? target.entry.executable || startup.executable || "" : target.installer || startup.executable || ""); setStep(repair ? 1 : -2);
  };
  const uninstall = async (options: UninstallOptions) => {
    if (!target || busy) return;
    if (preview) { setUninstallConfirmation(false); setError("Execute o binário Pelinstall para desinstalar."); return; }
    setUninstallConfirmation(false); setBusy(true); setError("");
    try { await uninstallPeliGamesEntry(target.entry.path, options); setRemoved(true); setStep(5); setMatches([]); }
    catch (error) { setError(String(error)); } finally { setBusy(false); }
  };
  const chooseFile = async () => {
    setError("");
    try { const selected = await openFilePicker(mode === "add" ? "Selecionar programa Windows" : mode === "repair" ? "Selecionar instalador para reparar" : "Selecionar instalador Windows", mode === "add" ? ["exe"] : ["exe", "msi"]);
      if (selected) setFile(await invoke<string>("validate_pelinstall_file", { path: selected }));
    } catch (error) { setError(String(error)); }
  };
  const chooseDirectory = async () => {
    setError("");
    // A new installation starts the system dialog in /mnt, where mounted disks live;
    // every folder stays reachable from there.
    try { const selected = await openDirectoryPicker(mode === "repair" ? "Selecionar prefixo Wine/Proton" : "Onde deseja instalar?", mode === "repair" ? undefined : "/mnt"); if (selected) { if (mode === "repair") setRepairPrefix(selected); else location.setDirectory(selected); } }
    catch (error) { setError(String(error)); }
  };
  const install = async () => {
    if (busy || (mode === "repair" && (shortcutChecking || shortcutCheckFailed)) || !validFile || !canResolveDirectory || !proton || !name.trim() || !mode) return;
    if (preview) { setError("A prévia permite testar as etapas. Execute o binário Pelinstall para iniciar uma instalação real."); return; }
    setBusy(true); setError("");
    if (mode !== "add" && (mode !== "repair" || proton !== target?.entry.proton || repairPrefix !== target?.entry.prefix)) setStep(3);
    let installed = false;
    try {
      if (mode === "repair") {
        const current = await lookupShortcuts();
        if (Boolean(current.length) !== Boolean(existingShortcuts.length)) {
          setExistingShortcuts(current); setDesktop(false); setRemoveExistingShortcuts(false); setUpdateExistingShortcuts(false);
          setStep(2); setError("Os atalhos foram alterados. Confira a opção de atalho antes de reparar."); return;
        }
      }
      // Renaming a game after choosing the default location needs a new, unique reservation.
      const installationDirectory = mode === "repair" ? directory : await location.resolveDirectory();
      const cover = await resolveCover(name);
      const request = { name: name.trim(), directory: installationDirectory, executable: file, proton, cover_url: cover || undefined };
      if (mode === "add") {
        const entry = await addPeliGamesEntry(request);
        installed = true;
        setResult({ prefix: entry.prefix || "", log: "", registered_count: 1 });
        setEntries([entry]); setChosen(entry.path); setStep(4);
        if (desktop) setShortcut(await invoke<string>("create_peligames_shortcut", { path: entry.path }));
        return;
      }
      const installation = mode === "repair" ? await repairPeliGamesEntry({ path: targetPath, name: name.trim(), proton, prefix: repairPrefix, executable: file }) : await runGameInstaller(request);
      installed = true;
      setResult(installation);
      if (mode === "repair" && cover && !installation.library_error) {
        try { await invoke("set_peligames_cover", { path: targetPath, coverUrl: cover }); }
        catch (error) { setError(`Reparação concluída, mas a capa não foi salva: ${String(error)}`); }
      }
      const found = (await listPeliGamesEntries()).filter(entry => entry.prefix === installation.prefix);
      const registered = mode === "repair" ? found.filter(entry => entry.path === targetPath) : found;
      const needsExecutableChoice = mode !== "repair" && !installation.library_error && registered.length > 0;
      setChoosingExecutable(needsExecutableChoice);
      setEntries(registered); setChosen(needsExecutableChoice ? "" : registered[0]?.path || "");
      if (installation.library_error) setError(installation.library_error);
      else if (!registered.length) setError("Nenhum executável foi encontrado. Abra o PeliGames para verificar a instalação e adicionar o executável.");
      setStep(4);
      if (mode === "repair" && !installation.library_error && removeExistingShortcuts) {
        setRemovedShortcutCount(await invoke<number>("remove_peligames_shortcuts", { path: targetPath })); setExistingShortcuts([]);
      }
      if (mode === "repair" && !installation.library_error && updateExistingShortcuts && !removeExistingShortcuts) {
        setUpdatedShortcutCount(await invoke<number>("update_peligames_shortcuts", { path: targetPath }));
      }
      if (!needsExecutableChoice && !installation.library_error && desktop && registered.length === 1) setShortcut(await invoke<string>("create_peligames_shortcut", { path: registered[0].path }));
    } catch (error) { setError(String(error)); setStep(installed ? 4 : 2); }
    finally {
      if (installed) window.dispatchEvent(new Event("peligamesLibraryChanged"));
      setBusy(false); setCancelPending(false);
      if (closeAfterCancel.current) { closeAfterCancel.current = false; await close(); }
    }
  };
  const confirmExecutable = async () => {
    if (!chosen || busy) return;
    setBusy(true); setError("");
    try {
      const game = completionPreview ? entries.find(entry => entry.path === chosen) ?? entries[0] : await choosePeliGamesEntry(chosen);
      setEntries([game]); setChosen(game.path); setChoosingExecutable(false);
      if (desktop && !shortcut) setShortcut(completionPreview ? "/preview/Desktop/peligames.desktop" : await invoke<string>("create_peligames_shortcut", { path: game.path }));
    } catch (error) { setError(String(error)); } finally { setBusy(false); }
  };
  const createShortcut = async () => {
    if (!chosen || busy) return;
    if (completionPreview) { setShortcut("/preview/Desktop/peligames.desktop"); return; }
    setBusy(true); setError("");
    try { setShortcut(await invoke<string>("create_peligames_shortcut", { path: chosen })); }
    catch (error) { setError(String(error)); } finally { setBusy(false); }
  };
  const openLauncherFromTitle = async () => {
    if (launcherOpening) return;
    if (preview) { window.open("/preview.html", "_blank"); return; }
    setLauncherOpening(true);
    try { await invoke("open_peligames_launcher"); }
    catch (error) { setError(String(error)); }
    finally { setLauncherOpening(false); }
  };
  const openLauncher = async () => {
    if (embedded) { await close(); return; }
    setError("");
    try { await invoke("open_peligames_launcher", { path: chosen || null }); stateRef.current.dirty = false; await close(); }
    catch (error) { setError(String(error)); }
  };
  // The window grows with its content (up to the tallest step) and shrinks back.
  useEffect(() => {
    if (preview || embedded || !panel.current) return;
    const scroller = panel.current.querySelector(".pi-body");
    if (!scroller) return;
    let frame = 0;
    const resize = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const children = Array.from(scroller.children).filter(child => getComputedStyle(child).display !== "none");
        const bottom = Math.max(...children.map(child => child.getBoundingClientRect().bottom));
        const height = Math.min(PELINSTALL_MAX_HEIGHT, Math.max(260, Math.ceil(bottom - panel.current!.getBoundingClientRect().top + scroller.scrollTop + 22)));
        if (Math.abs(height - window.innerHeight) > 1) void getCurrentWindow().setSize(new LogicalSize(window.innerWidth, height)).catch(error => setError(String(error)));
      });
    };
    const observer = new ResizeObserver(resize);
    Array.from(scroller.children).forEach(child => observer.observe(child));
    resize();
    return () => { observer.disconnect(); cancelAnimationFrame(frame); };
  }, [step, error, result, shortcut, location.selected, checking, matches.length, choosingExecutable, entries.length]);
  // Which of the two location cards produced the current folder.
  const [placeKind, setPlaceKind] = useState<"default" | "custom" | null>(null);
  useEffect(() => { if (!location.hasSelection) setPlaceKind(null); }, [location.hasSelection]);
  const backToStart = () => { if (embedded) { requestClose(); return; } setMode(null); setName(""); setProton(""); location.reset(); setRepairPrefix(""); setFile(startup.executable || ""); setError(""); setStep(-1); };
  const back = () => { setError(""); if (step === -2 || (mode === "repair" && step === 1)) backToStart(); else setStep(step === 0 ? -2 : step - 1); };
  const fileName = file.split(/[\\/]/).pop() || "";
  const shownName = name.trim() || (step === -1 && target ? target.entry.name : fileName) || tr("Nenhum executável selecionado");
  const shownPath = step === -1 && target ? target.entry.executable || startup.executable || "" : file;
  const chip = step === 3 ? [tr("Em andamento"), ""] : step === 4 && !error ? [mode === "repair" ? tr("Reparado") : tr("Na biblioteca"), "ok"] : step === -1 && target ? [tr("Na biblioteca"), "ok"] : step === 5 ? [tr("Removido"), ""] : [mode === "add" ? tr("Programa Windows") : "Windows · .exe", ""];
  const fileCard = <div className="pi-file">
    <div className="pi-file-icon">{executableIcon ? <img src={executableIcon} alt={tr("Ícone do executável")} /> : <MenuIcon name="document" />}</div>
    <div className="pi-file-text"><strong title={shownName}>{shownName}</strong>{shownPath && <small title={shownPath}>{shownPath}</small>}</div>
    <span className={`pi-chip ${chip[1]}`}>{chip[0]}</span>
  </div>;
  const stepLabels = [tr("Nome"), tr("Local"), "Proton", tr("Revisão"), mode === "add" ? tr("Adicionar") : mode === "repair" ? tr("Reparar") : tr("Instalar")];
  const progressStep = step === -2 ? 0 : step + 1;
  const progress = (step === -2 || step >= 0) && step <= 3 && <div className="pi-steps-wrap" aria-hidden="true">
    <div className="pi-steps">{stepLabels.map((label, index) => <i key={label} className={index < progressStep ? "done" : index === progressStep ? "now" : ""} />)}</div>
    <div className="pi-step-labels">{stepLabels.map((label, index) => <span key={label} className={index === progressStep ? "now" : ""}>{label}</span>)}</div>
  </div>;
  const icon = (name: Parameters<typeof MenuIcon>[0]["name"], from: string, to: string) => <InstallationActionIcon name={name} from={from} to={to}/>;
  const card = (key: string, iconNode: ReactNode, title: string, hint: string, onClick: () => void, options: { pressed?: boolean; danger?: boolean; disabled?: boolean } = {}) =>
    <button type="button" key={key} className={`pi-card${options.danger ? " danger" : ""}`} aria-pressed={options.pressed} disabled={options.disabled} onClick={onClick}>{iconNode}<span className="pi-card-copy"><strong>{title}</strong><small>{hint}</small></span></button>;
  const nextDisabled = busy || (step === -2 ? !name.trim() || !validFile : step === 0 ? !validFile || !directorySelected || !directory.trim() : !proton);
  const finishDisabled = busy || (mode === "repair" && (shortcutChecking || shortcutCheckFailed)) || !name.trim() || !validFile || !proton || !directory.trim() || !canResolveDirectory;
  const repairShortcuts = mode === "repair" && existingShortcuts.length > 0;
  const launchChosen = () => { if (preview) { setError("Execute o binário Pelinstall para iniciar o programa."); return; } setBusy(true); setError(""); void invoke("launch_pelinstall_entry", { path: chosen }).then(() => close()).catch(error => setError(String(error))).finally(() => setBusy(false)); };
  const shownInPrefix = (path?: string | null) => (result?.prefix && path?.startsWith(result.prefix) ? path.slice(result.prefix.length).replace(/^\/drive_c\//, "C:/") : path) ?? "";

  return <main ref={panel} className={`pelinstall-window pi-window${embedded ? " pi-embedded" : ""}`}>
    <header className="pi-titlebar" onMouseDown={event => { if (!embedded) dragPelinstallWindow(event, setError); }}>
      <h1 className="pi-brand"><img src="/peligames.svg" alt="" /><span className="pelinstall-brand">Pel<span className="titlebar-games">install</span></span></h1>
      <span className="pi-spacer" />
      {!embedded && <InstallerSettingsMenu />}
      {!embedded && <button type="button" className="titlebar-title pelinstall-launcher-button" aria-label={tr("Abrir PeliGames")} title={tr("Abrir PeliGames")} disabled={launcherOpening} onClick={() => void openLauncherFromTitle()}><BrandName /></button>}
      {!embedded && <PelinstallMinimizeButton onError={setError} />}
      <button type="button" className="btn-titlebar window-control pi-close" aria-label={tr("Fechar")} title={tr("Fechar")} disabled={(busy && step !== 3) || cancelPending} onClick={requestClose}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12" /></svg>
      </button>
    </header>
    <div className="pi-body">
      {fileCard}
      {progress}

      {step === -1 && checking && <p className="pi-hint" role="status">{tr("Consultando a biblioteca…")}</p>}
      {step === -1 && !checking && !matches.length && <>
        <h2>{tr("O que deseja fazer?")}</h2>
        <div className="pi-cards">
          {card("install", icon("download", "#5aa9ff", "#2058d8"), tr("Instalar"), tr("Abre o instalador com o Proton e registra o jogo na biblioteca."), () => { setMode("install"); setError(""); setStep(-2); })}
          {card("add", icon("plusCircle", "#3ee0a6", "#06946a"), tr("Adicionar"), tr("O jogo já está instalado. Só cria o registro e o prefixo."), () => { setMode("add"); setError(""); setStep(-2); })}
        </div>
      </>}
      {step === -1 && !checking && target && <>
        <div className="pi-game">
          {target.entry.cover_url ? <div className="pi-game-cover" style={{ backgroundImage: `url("${target.entry.cover_url.replace(/"/g, "%22")}")` }} /> : <div className="pi-game-cover empty"><MenuIcon name="gamepad" /></div>}
          <div className="pi-file-text"><strong>{target.entry.name}</strong><small title={target.entry.prefix}>{target.entry.prefix}{target.entry.proton ? ` · ${target.entry.proton.split(/[\\/]/).pop()}` : ""}</small></div>
        </div>
        {matches.length > 1 && <EntrySelector label={tr("Registro na biblioteca")} value={target.entry.path} onChange={setTargetPath} options={matches.map((match, index) => ({ value: match.entry.path, label: `${index + 1}. ${match.entry.name}` }))} />}
        <h2>{tr("Já está na biblioteca")}</h2>
        <div className="pi-cards">
          {card("repair", icon("repair", "#ffb54d", "#ef6a17"), tr("Reparar"), tr("Atualiza o prefixo, o Proton e os atalhos."), () => beginExisting(true), { disabled: busy })}
          {card("duplicate", icon("copy", "#c28dff", "#7432e8"), tr("Duplicar"), tr("Instala de novo em outro prefixo."), () => beginExisting(false), { disabled: busy })}
          {card("settings", icon("settings", "#a3abbd", "#555e73"), tr("Configurações"), tr("Abre o jogo no PeliGames."), () => void openSettings(), { disabled: busy })}
          {card("uninstall", icon("trash", "#ff7f8e", "#d1293f"), tr("Desinstalar"), tr("Escolha o que apagar: prefixo, atalhos, saves."), () => setUninstallConfirmation(true), { danger: true, disabled: busy })}
        </div>
      </>}

      {step === -2 && <>
        <h2>{tr("Qual é o nome do jogo ou programa?")}</h2>
        <div className="pi-field"><input aria-label={tr("Nome")} placeholder={tr("Digite o nome")} value={name} onChange={event => setName(event.target.value)} disabled={busy} /></div>
        <p className="pi-hint">{tr("A pasta padrão do prefixo usará este nome. Na próxima etapa, escolha onde ela será criada.")}</p>
        {!validFile && <button type="button" className="pi-btn" onClick={() => void chooseFile()}><MenuIcon name="folderSearch" />{tr(mode === "install" ? "Selecionar instalador" : "Selecionar executável")}</button>}
      </>}

      {step === 0 && <>
        <h2>{mode === "repair" ? tr("Qual prefixo deseja usar para reparar?") : mode === "add" ? tr("Onde deseja criar o prefixo?") : tr("Onde deseja instalar?")}</h2>
        {!validFile && <button type="button" className="pi-btn" onClick={() => void chooseFile()}><MenuIcon name="folderSearch" />{mode === "repair" ? tr("Selecionar instalador") : tr("Selecionar executável")}</button>}
        {mode !== "repair" && <div className="pi-cards">
          {card("default", icon("download", "var(--pi-accent-hi)", "var(--pi-accent-lo)"), tr("Pasta padrão"), tr("Pasta de jogos do PeliGames, definida nas Configurações."), () => void location.selectDefault().then(() => setPlaceKind("default")).catch(error => setError(String(error))), { pressed: placeKind === "default" })}
          {card("custom", icon("folder", "#ffb54d", "#ef6a17"), tr("Escolher outra"), tr("Abre o seletor de pastas do sistema, começando em /mnt."), () => void chooseDirectory().then(() => { if (location.selected) setPlaceKind("custom"); }), { pressed: placeKind === "custom" })}
        </div>}
        {directorySelected ? <>
          <div className="pi-label">{mode === "repair" ? tr("Prefixo") : tr("Local escolhido")}</div>
          <HoverTooltip text={directory} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
            <div className="pi-field"><input aria-label={tr("Local de instalação")} value={directory} onChange={event => { if (mode === "repair") setRepairPrefix(event.target.value); else location.setDirectory(event.target.value); }} />
              <button type="button" className="pi-mini" aria-label={tr("Alterar local de instalação")} title={tr("Alterar local de instalação")} onClick={() => void chooseDirectory().then(() => { if (mode !== "repair") setPlaceKind("custom"); })}><MenuIcon name="folderSearch" /></button></div>
          </HoverTooltip>
        </> : mode === "repair" ? <button type="button" className="pi-btn" onClick={() => void chooseDirectory()}><MenuIcon name="folderSearch" />{tr("Escolher prefixo")}</button>
          : <p className="pi-hint">{tr("Escolha uma das opções para continuar.")}</p>}
      </>}

      {step === 1 && <>
        <h2>{mode === "add" ? tr("Executar com qual Proton?") : mode === "repair" ? tr("Reparar com qual Proton?") : tr("Instalar com qual Proton?")}</h2>
        <PelinstallProtonPicker value={proton} onChange={setProton} disabled={busy} onManage={() => setProtonPreferencesOpen(true)} />
      </>}

      {step === 2 && <>
        <h2>{tr("Confira antes de continuar")}</h2>
        {mode === "repair" && <div className="pi-field"><input aria-label={tr("Nome")} placeholder={tr("Digite o nome")} value={name} onChange={event => setName(event.target.value)} disabled={busy} /></div>}
        {mode === "repair" && <><div className="pi-label">{tr("Executável")}</div><EntrySelector label={tr("Executável para reparar")} value={file} onChange={setFile} disabled={busy} options={Array.from(new Map([...(target ? [target.entry] : []), ...repairExecutables].map(entry => [entry.executable || "", { value: entry.executable || "", label: entry.executable?.split(/[\\/]/).pop() || entry.name }])).values()).filter(option => option.value)} /></>}
        <div className="pi-toggle">
          <span>{mode === "repair" && shortcutChecking ? tr("Verificando atalhos…") : mode === "repair" && shortcutCheckFailed ? tr("Não foi possível verificar os atalhos") : repairShortcuts ? tr("Remover atalho da área de trabalho") : tr("Criar atalho na área de trabalho")}
            <small>{repairShortcuts ? tr("Este jogo já tem atalho na área de trabalho.") : tr("Abre o jogo direto, sem abrir o PeliGames.")}</small></span>
          <button type="button" className="pi-switch" role="switch" aria-label={repairShortcuts ? tr("Remover atalho da área de trabalho") : tr("Criar atalho na área de trabalho")} aria-checked={repairShortcuts ? removeExistingShortcuts : desktop}
            disabled={busy || (mode === "repair" && (shortcutChecking || shortcutCheckFailed))}
            onClick={() => { if (repairShortcuts) { const next = !removeExistingShortcuts; setRemoveExistingShortcuts(next); if (next) setUpdateExistingShortcuts(false); } else setDesktop(!desktop); }} />
        </div>
        {repairShortcuts && <div className="pi-toggle">
          <span>{tr("Atualizar atalho ao reparar")}<small>{tr("Aponta o atalho para o executável e o Proton escolhidos.")}</small></span>
          <button type="button" className="pi-switch" role="switch" aria-label={tr("Atualizar atalho ao reparar")} aria-checked={updateExistingShortcuts} disabled={busy || shortcutChecking || shortcutCheckFailed}
            onClick={() => { setUpdateExistingShortcuts(!updateExistingShortcuts); setRemoveExistingShortcuts(false); }} />
        </div>}
        <dl className="pi-summary"><dt>{tr("Destino")}</dt><dd title={directory}>{directory}</dd><dt>Proton</dt><dd title={proton}>{proton.split(/[\\/]/).pop()}</dd></dl>
        <p className="pi-hint">{mode === "add" ? tr("O executável será adicionado à biblioteca com o prefixo e o Proton selecionados.") : mode === "repair" ? tr("O prefixo será atualizado se o Proton escolhido for diferente do atual.") : tr("O instalador Windows abrirá suas próprias telas. Finalize a instalação nele para registrar os executáveis na biblioteca.")}</p>
      </>}

      {step === 3 && <div className="pi-working" role="status">
        <div className="pi-indeterminate"><i /></div>
        <h2>{cancelPending ? tr("Encerrando os processos…") : mode === "repair" ? tr("Atualizando o prefixo") : tr("Aguardando o instalador do Windows")}</h2>
        <p className="pi-hint">{mode === "repair" ? tr("A conclusão será liberada quando o Proton terminar de preparar o prefixo.") : tr("Termine a instalação nas telas do próprio instalador. O Pelinstall fica aberto e registra o jogo quando ele fechar.")}</p>
      </div>}

      {step === 4 && choosingExecutable && <>
        <h2>{t("gameModes", "chooseExecutableTitle")}</h2>
        <p className="pi-hint">{t("gameModes", "chooseExecutableHint")}</p>
        <div className="pi-list pi-scroll4" role="radiogroup" aria-label={t("gameModes", "chooseExecutableTitle")}>
          {entries.map(entry => <button type="button" key={entry.path} className="pi-option" role="radio" aria-checked={chosen === entry.path} disabled={busy} onClick={() => setChosen(entry.path)}>
            <span className="pi-radio" /><span className="pi-option-text"><strong>{(entry.executable ?? entry.path).split("/").pop()}</strong><small>{shownInPrefix(entry.executable)}</small></span>
            {(entry.recommended || entry.discovery_source) && <span className="pi-tag" title={entry.discovery_source ?? undefined}>{entry.recommended ? t("gameModes", "chooseExecutableRecommended") : entry.discovery_source}</span>}
          </button>)}
        </div>
        <div className="pi-actions">
          <button type="button" className="pi-btn" disabled={busy} onClick={() => setChoosingExecutable(false)}>{t("gameModes", "chooseExecutableKeepAll")}</button>
          <button type="button" className="pi-btn primary grow" disabled={busy || !chosen} onClick={() => void confirmExecutable()}>{t("gameModes", "chooseExecutableConfirm")}</button>
        </div>
      </>}
      {step === 4 && !choosingExecutable && <>
        <div className="pi-done">
          <span className={`pi-check${error ? " warn" : ""}`}><MenuIcon name={error ? "info" : "check"} /></span>
          <div><h2>{mode === "add" ? tr("Adicionado à biblioteca") : mode === "repair" ? tr("Reparação finalizada") : tr("Instalação finalizada")}</h2>
            <p className="pi-hint">{entries.length === 1 ? tr("Pronto para jogar pelo PeliGames ou pelo atalho.") : `${entries.length} ${tr("executável(is) registrado(s) no PeliGames.")}`}</p></div>
        </div>
        {entries.length > 1 && <><div className="pi-label">{tr("Qual executável deseja usar?")}</div>
          <EntrySelector label={tr("Qual executável deseja usar?")} value={chosen} onChange={setChosen} disabled={busy} options={entries.map(entry => ({ value: entry.path, label: `${entry.name} — ${entry.executable?.split(/[\\/]/).pop()}${entry.discovery_source ? ` (${entry.discovery_source})` : ""}` }))} /></>}
        {entries.length > 0 && <button type="button" className="pi-play" disabled={!chosen || busy} onClick={launchChosen}><MenuIcon name="play" />{tr("Jogar agora")}</button>}
        {desktop && !shortcut && entries.length > 1 && <button type="button" className="pi-btn" disabled={!chosen || busy} onClick={() => void createShortcut()}>{tr("Criar atalho")}</button>}
        <div className="pi-notes">
          {shortcut && <p title={shortcut}><MenuIcon name="check" />{tr("Atalho criado na área de trabalho")}</p>}
          {updatedShortcutCount > 0 && <p><MenuIcon name="check" />{tr("Atalho atualizado.")}</p>}
          {removedShortcutCount > 0 && <p><MenuIcon name="check" />{removedShortcutCount === 1 ? tr("Atalho removido.") : tr("Atalhos removidos.")}</p>}
          {result?.log && <p title={result.log}>Log: {result.log}</p>}
        </div>
      </>}

      {removed && step === 5 && <><h2>{tr("Prefixo desinstalado")}</h2><p className="pi-hint">{tr("Os registros que utilizavam esse prefixo foram removidos da biblioteca.")}</p></>}

      {error && <p className="pi-error" role="alert">{error}</p>}

      {(step === -2 || step >= 0 && step < 3) && <div className="pi-actions">
        <button type="button" className="pi-btn" disabled={busy} onClick={back}><MenuIcon name="back" />{tr("Voltar")}</button>
        {step < 2 ? <button type="button" className="pi-btn primary grow" disabled={nextDisabled} onClick={() => { setError(""); setStep(step === -2 ? 0 : step + 1); }}>{tr("Próximo")} <MenuIcon name="play" /></button>
          : <button type="button" className="pi-btn primary grow" disabled={finishDisabled} onClick={() => void install()}><MenuIcon name={mode === "add" ? "folder" : mode === "repair" ? "repair" : "download"} />{busy ? mode === "add" ? tr("Adicionando…") : tr("Salvando…") : mode === "add" ? tr("Adicionar") : mode === "repair" ? tr("Reparar") : tr("Instalar")}</button>}
      </div>}
      {step === 3 && <div className="pi-actions"><button type="button" className="pi-btn danger grow" disabled={cancelPending} onClick={() => { closeAfterCancel.current = false; setCancelConfirmation(true); }}>{cancelPending ? tr("Cancelando…") : mode === "repair" ? tr("Cancelar reparação") : tr("Cancelar instalação")}</button></div>}
      {((step === 4 && !choosingExecutable) || step === 5) && <div className="pi-actions">
        <button type="button" className="pi-btn grow" disabled={busy} onClick={() => void close()}>{tr("Concluir")}</button>
        <button type="button" className="pi-btn grow" disabled={busy} onClick={() => void openLauncher()}>{tr("Abrir no PeliGames")}</button>
      </div>}
    </div>
    {uninstallConfirmation && target && <PrefixUninstallModal name={target.entry.name} prefix={target.entry.prefix || ""} busy={busy} onConfirm={options => void uninstall(options)} onCancel={() => setUninstallConfirmation(false)} />}
    {cancelConfirmation && <ConfirmModal title={mode === "repair" ? tr("Cancelar reparação?") : tr("Cancelar instalação?")} message={mode === "repair" ? tr("Deseja encerrar a atualização do prefixo? Os arquivos e logs serão preservados, mas a preparação pode ficar incompleta. As alterações do registro não serão salvas.") : tr("Deseja encerrar o instalador e os processos deste prefixo? A instalação pode ficar incompleta. O prefixo e os logs serão preservados e nenhum jogo será registrado por esta tentativa.")} confirmText={tr("Encerrar instalação")} onConfirm={() => void cancelInstallation()} onCancel={() => { closeAfterCancel.current = false; setCancelConfirmation(false); }} />}
    {exitConfirmation && <ConfirmModal title={tr("Configuração incompleta")} message={tr("Deseja sair e descartar a configuração iniciada?")} onConfirm={() => { stateRef.current.dirty = false; setExitConfirmation(false); void close(); }} onCancel={() => setExitConfirmation(false)} />}
    {protonPreferencesOpen && <Suspense fallback={null}><ProtonPreferencesDialog overlayLayer={embedded ? 2300 : undefined} onClose={() => setProtonPreferencesOpen(false)} onBusyChange={setProtonPreferencesBusy} /></Suspense>}
  </main>;
}

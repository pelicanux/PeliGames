import { useRemovedPrefixNotice } from "../../components/RemovedPrefixNotice";
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
  const removedPrefixNotice=useRemovedPrefixNotice(location.trackReservation);
  const directory = mode === "repair" ? repairPrefix : location.directory;
  const directorySelected = mode === "repair" ? Boolean(repairPrefix.trim()) : location.selected;
  const canResolveDirectory = mode === "repair" ? directorySelected : location.hasSelection;
  const validFile = Boolean(file && (mode !== "add" || file.toLowerCase().endsWith(".exe")));
  const stateRef = useRef({ busy, dirty: false, installing: false });
  const closing = useRef(false);
  stateRef.current = { busy: busy || protonPreferencesBusy, dirty: !result && Boolean(directorySelected || proton || name.trim()), installing: busy && step === 3 };
  const close = async () => {
    await location.release();
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
  const removeOnly=async()=>{
    if(!target||busy)return;
    if(preview){setError("A prévia não remove entradas reais.");return;}
    setBusy(true);setError("");
    try{await invoke("remove_peligames_library_entry",{path:target.entry.path});setUninstallConfirmation(false);window.dispatchEvent(new Event("peligamesLibraryChanged"));await close();}
    catch(error){setError(String(error));}finally{setBusy(false);}
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
      if(mode!=="repair"&&!await removedPrefixNotice.inspect(installationDirectory,file))return;
    if (mode !== "add" && (mode !== "repair" || proton !== target?.entry.proton || repairPrefix !== target?.entry.prefix)) setStep(3);
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
  const shownName = name.trim() || (step === -1 && target ? target.entry.name : fileName) || tr("no_executable_selected");
  const shownPath = step === -1 && target ? target.entry.executable || startup.executable || "" : file;
  const chip = step === 3 ? [tr("in_progress"), ""] : step === 4 && !error ? [mode === "repair" ? tr("repaired") : tr("in_the_library"), "ok"] : step === -1 && target ? [tr("in_the_library"), "ok"] : step === 5 ? [tr("removed"), ""] : [mode === "add" ? tr("windows_program") : "Windows · .exe", ""];
  const fileCard = <div className="pi-file">
    <div className="pi-file-icon">{executableIcon ? <img src={executableIcon} alt={tr("executable_icon")} /> : <MenuIcon name="document" />}</div>
    <div className="pi-file-text"><strong title={shownName}>{shownName}</strong>{shownPath && <small title={shownPath}>{shownPath}</small>}</div>
    <span className={`pi-chip ${chip[1]}`}>{chip[0]}</span>
  </div>;
  const stepLabels = [tr("name"), tr("location"), "Proton", tr("review"), mode === "add" ? tr("add") : mode === "repair" ? tr("repair") : tr("install")];
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
      {!embedded && <button type="button" className="titlebar-title pelinstall-launcher-button" aria-label={tr("open_peligames")} title={tr("open_peligames")} disabled={launcherOpening} onClick={() => void openLauncherFromTitle()}><BrandName /></button>}
      {!embedded && <PelinstallMinimizeButton onError={setError} />}
      <button type="button" className="btn-titlebar window-control pi-close" aria-label={tr("close")} title={tr("close")} disabled={(busy && step !== 3) || cancelPending} onClick={requestClose}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12" /></svg>
      </button>
    </header>
    <div className="pi-body">
      {fileCard}
      {progress}

      {step === -1 && checking && <p className="pi-hint" role="status">{tr("checking_the_library")}</p>}
      {step === -1 && !checking && !matches.length && <>
        <h2>{tr("what_do_you_want_to_do")}</h2>
        <div className="pi-cards">
          {card("install", icon("download", "#5aa9ff", "#2058d8"), tr("install"), tr("runs_the_installer_with_proton_and_adds_the_game"), () => { setMode("install"); setError(""); setStep(-2); })}
          {card("add", icon("plusCircle", "#3ee0a6", "#06946a"), tr("add"), tr("the_game_is_already_installed_only_creates_the_entry"), () => { setMode("add"); setError(""); setStep(-2); })}
        </div>
      </>}
      {step === -1 && !checking && target && <>
        <div className="pi-game">
          {target.entry.cover_url ? <div className="pi-game-cover" style={{ backgroundImage: `url("${target.entry.cover_url.replace(/"/g, "%22")}")` }} /> : <div className="pi-game-cover empty"><MenuIcon name="gamepad" /></div>}
          <div className="pi-file-text"><strong>{target.entry.name}</strong><small title={target.entry.prefix}>{target.entry.prefix}{target.entry.proton ? ` · ${target.entry.proton.split(/[\\/]/).pop()}` : ""}</small></div>
        </div>
        {matches.length > 1 && <EntrySelector label={tr("library_entry")} value={target.entry.path} onChange={setTargetPath} options={matches.map((match, index) => ({ value: match.entry.path, label: `${index + 1}. ${match.entry.name}` }))} />}
        <h2>{tr("already_in_the_library")}</h2>
        <div className="pi-cards">
          {card("repair", icon("repair", "#ffb54d", "#ef6a17"), tr("repair"), tr("updates_the_prefix_proton_and_shortcuts"), () => beginExisting(true), { disabled: busy })}
          {card("duplicate", icon("copy", "#c28dff", "#7432e8"), tr("duplicate"), tr("installs_again_in_another_prefix"), () => beginExisting(false), { disabled: busy })}
          {card("settings", icon("settings", "#a3abbd", "#555e73"), tr("settings"), tr("opens_the_game_in_peligames"), () => void openSettings(), { disabled: busy })}
          {card("uninstall", icon("trash", "#ff7f8e", "#d1293f"), tr("uninstall"), tr("choose_what_to_delete_prefix_shortcuts_saves"), () => setUninstallConfirmation(true), { danger: true, disabled: busy })}
        </div>
      </>}

      {step === -2 && <>
        <h2>{tr("what_is_the_name_of_the_game_or_program")}</h2>
        <div className="pi-field"><input aria-label={tr("name")} placeholder={tr("enter_a_name")} value={name} onChange={event => setName(event.target.value)} disabled={busy} /></div>
        <p className="pi-hint">{tr("the_default_prefix_folder_will_use_this_name_choose")}</p>
        {!validFile && <button type="button" className="pi-btn" onClick={() => void chooseFile()}><MenuIcon name="folderSearch" />{tr(mode === "install" ? "select_installer" : "select_executable")}</button>}
      </>}

      {step === 0 && <>
        <div className="pi-location-heading">
          <h2>{mode === "repair" ? tr("which_prefix_should_be_repaired") : mode === "add" ? tr("where_should_the_prefix_be_created") : tr("where_should_it_be_installed")}</h2>
          {mode === "add" && <p className="pi-hint">{tr("a_folder_with_the_windows_environment_used_by_the")}</p>}
        </div>
        {!validFile && <button type="button" className="pi-btn" onClick={() => void chooseFile()}><MenuIcon name="folderSearch" />{mode === "repair" ? tr("select_installer") : tr("select_executable")}</button>}
        {mode !== "repair" && <div className="pi-cards">
          {card("default", icon("download", "var(--pi-accent-hi)", "var(--pi-accent-lo)"), tr("default_folder"), tr("peligames_games_folder_set_in_settings"), () => void location.selectDefault().then(() => setPlaceKind("default")).catch(error => setError(String(error))), { pressed: placeKind === "default" })}
          {card("custom", icon("folder", "#ffb54d", "#ef6a17"), tr("choose_another"), tr("opens_the_system_folder_picker_starting_in_mnt"), () => void chooseDirectory().then(() => { if (location.selected) setPlaceKind("custom"); }), { pressed: placeKind === "custom" })}
        </div>}
        {directorySelected ? <>
          <div className="pi-label">{mode === "repair" ? tr("prefix") : tr("chosen_location")}</div>
          <HoverTooltip text={directory} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
            <div className="pi-field"><input aria-label={tr("installation_location")} value={directory} onChange={event => { if (mode === "repair") setRepairPrefix(event.target.value); else location.setDirectory(event.target.value); }} />
              <button type="button" className="pi-mini" aria-label={tr("change_installation_location")} title={tr("change_installation_location")} onClick={() => void chooseDirectory().then(() => { if (mode !== "repair") setPlaceKind("custom"); })}><MenuIcon name="folderSearch" /></button></div>
          </HoverTooltip>
        </> : mode === "repair" ? <button type="button" className="pi-btn" onClick={() => void chooseDirectory()}><MenuIcon name="folderSearch" />{tr("choose_prefix")}</button>
          : <p className="pi-hint">{tr("choose_one_of_the_options_to_continue")}</p>}
      </>}

      {step === 1 && <>
        <h2>{mode === "add" ? tr("run_with_which_proton") : mode === "repair" ? tr("repair_with_which_proton") : tr("install_with_which_proton")}</h2>
        <PelinstallProtonPicker value={proton} onChange={setProton} disabled={busy} onManage={() => setProtonPreferencesOpen(true)} />
      </>}

      {step === 2 && <>
        <h2>{tr("review_before_continuing")}</h2>
        {mode === "repair" && <div className="pi-field"><input aria-label={tr("name")} placeholder={tr("enter_a_name")} value={name} onChange={event => setName(event.target.value)} disabled={busy} /></div>}
        {mode === "repair" && <><div className="pi-label">{tr("executable")}</div><EntrySelector label={tr("executable_to_repair")} value={file} onChange={setFile} disabled={busy} options={Array.from(new Map([...(target ? [target.entry] : []), ...repairExecutables].map(entry => [entry.executable || "", { value: entry.executable || "", label: entry.executable?.split(/[\\/]/).pop() || entry.name }])).values()).filter(option => option.value)} /></>}
        <div className="pi-toggle">
          <span>{mode === "repair" && shortcutChecking ? tr("checking_shortcuts") : mode === "repair" && shortcutCheckFailed ? tr("could_not_check_shortcuts") : repairShortcuts ? tr("remove_desktop_shortcut") : tr("create_desktop_shortcut")}
            <small>{repairShortcuts ? tr("this_game_already_has_a_desktop_shortcut") : tr("starts_the_game_directly_without_opening_peligames")}</small></span>
          <button type="button" className="pi-switch" role="switch" aria-label={repairShortcuts ? tr("remove_desktop_shortcut") : tr("create_desktop_shortcut")} aria-checked={repairShortcuts ? removeExistingShortcuts : desktop}
            disabled={busy || (mode === "repair" && (shortcutChecking || shortcutCheckFailed))}
            onClick={() => { if (repairShortcuts) { const next = !removeExistingShortcuts; setRemoveExistingShortcuts(next); if (next) setUpdateExistingShortcuts(false); } else setDesktop(!desktop); }} />
        </div>
        {repairShortcuts && <div className="pi-toggle">
          <span>{tr("update_shortcut_when_repairing_2")}<small>{tr("points_the_shortcut_to_the_chosen_executable_and_proton")}</small></span>
          <button type="button" className="pi-switch" role="switch" aria-label={tr("update_shortcut_when_repairing_2")} aria-checked={updateExistingShortcuts} disabled={busy || shortcutChecking || shortcutCheckFailed}
            onClick={() => { setUpdateExistingShortcuts(!updateExistingShortcuts); setRemoveExistingShortcuts(false); }} />
        </div>}
        <dl className="pi-summary"><dt>{tr("destination_2")}</dt><dd title={directory}>{directory}</dd><dt>Proton</dt><dd title={proton}>{proton.split(/[\\/]/).pop()}</dd></dl>
        <p className="pi-hint">{mode === "add" ? tr("the_executable_will_be_added_to_the_library_with") : mode === "repair" ? tr("the_prefix_will_be_updated_if_the_selected_proton") : tr("the_windows_installer_will_open_its_own_windows_finish")}</p>
      </>}

      {step === 3 && <div className="pi-working" role="status">
        <div className="pi-indeterminate"><i /></div>
        <h2>{cancelPending ? tr("stopping_processes") : mode === "repair" ? tr("updating_the_prefix") : tr("waiting_for_the_windows_installer")}</h2>
        <p className="pi-hint">{mode === "repair" ? tr("you_can_finish_when_proton_has_prepared_the_prefix") : tr("finish_the_installation_in_the_installer_s_own_windows")}</p>
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
          <div><h2>{mode === "add" ? tr("added_to_the_library") : mode === "repair" ? tr("repair_complete") : tr("installation_complete")}</h2>
            <p className="pi-hint">{entries.length === 1 ? tr("ready_to_play_from_peligames_or_the_shortcut") : `${entries.length} ${tr("executable_s_registered_in_peligames")}`}</p></div>
        </div>
        {entries.length > 1 && <><div className="pi-label">{tr("which_executable_should_be_used")}</div>
          <EntrySelector label={tr("which_executable_should_be_used")} value={chosen} onChange={setChosen} disabled={busy} options={entries.map(entry => ({ value: entry.path, label: `${entry.name} — ${entry.executable?.split(/[\\/]/).pop()}${entry.discovery_source ? ` (${entry.discovery_source})` : ""}` }))} /></>}
        {entries.length > 0 && <button type="button" className="pi-play" disabled={!chosen || busy} onClick={launchChosen}><MenuIcon name="play" />{tr("play_now")}</button>}
        {desktop && !shortcut && entries.length > 1 && <button type="button" className="pi-btn" disabled={!chosen || busy} onClick={() => void createShortcut()}>{tr("create_shortcut")}</button>}
        <div className="pi-notes">
          {shortcut && <p title={shortcut}><MenuIcon name="check" />{tr("desktop_shortcut_created")}</p>}
          {updatedShortcutCount > 0 && <p><MenuIcon name="check" />{tr("shortcut_updated")}</p>}
          {removedShortcutCount > 0 && <p><MenuIcon name="check" />{removedShortcutCount === 1 ? tr("shortcut_removed") : tr("shortcuts_removed")}</p>}
          {result?.log && <p title={result.log}>Log: {result.log}</p>}
        </div>
      </>}

      {removed && step === 5 && <><h2>{tr("prefix_uninstalled")}</h2><p className="pi-hint">{tr("entries_using_this_prefix_were_removed_from_the_library")}</p></>}

      {error && <p className="pi-error" role="alert">{error}</p>}

      {(step === -2 || step >= 0 && step < 3) && <div className="pi-actions">
        <button type="button" className="pi-btn" disabled={busy} onClick={back}><MenuIcon name="back" />{tr("back")}</button>
        {step < 2 ? <button type="button" className="pi-btn primary grow" disabled={nextDisabled} onClick={() => { setError(""); setStep(step === -2 ? 0 : step + 1); }}>{tr("next")} <MenuIcon name="play" /></button>
          : <button type="button" className="pi-btn primary grow" disabled={finishDisabled} onClick={() => void install()}><MenuIcon name={mode === "add" ? "folder" : mode === "repair" ? "repair" : "download"} />{busy ? mode === "add" ? tr("adding") : tr("saving") : mode === "add" ? tr("add") : mode === "repair" ? tr("repair") : tr("install")}</button>}
      </div>}
      {step === 3 && <div className="pi-actions"><button type="button" className="pi-btn danger grow" disabled={cancelPending} onClick={() => { closeAfterCancel.current = false; setCancelConfirmation(true); }}>{cancelPending ? tr("cancelling") : mode === "repair" ? tr("cancel_repair") : tr("cancel_installation")}</button></div>}
      {((step === 4 && !choosingExecutable) || step === 5) && <div className="pi-actions">
        <button type="button" className="pi-btn grow" disabled={busy} onClick={() => void close()}>{tr("finish")}</button>
        <button type="button" className="pi-btn grow" disabled={busy} onClick={() => void openLauncher()}>{tr("open_in_peligames")}</button>
      </div>}
    </div>
    {uninstallConfirmation && target && <PrefixUninstallModal name={target.entry.name} prefix={target.entry.prefix || ""} busy={busy} error={error} onRemoveOnly={()=>void removeOnly()} onConfirm={options => void uninstall(options)} onCancel={() => setUninstallConfirmation(false)} />}
    {cancelConfirmation && <ConfirmModal title={mode === "repair" ? tr("cancel_repair_2") : tr("cancel_installation_2")} message={mode === "repair" ? tr("stop_updating_the_prefix_files_and_logs_will_be") : tr("stop_the_installer_and_this_prefix_s_processes_installation")} confirmText={tr("stop_installation")} onConfirm={() => void cancelInstallation()} onCancel={() => { closeAfterCancel.current = false; setCancelConfirmation(false); }} />}
    {removedPrefixNotice.dialog}
    {exitConfirmation && <ConfirmModal title={tr("incomplete_configuration")} message={tr("exit_and_discard_the_current_configuration")} onConfirm={() => { stateRef.current.dirty = false; setExitConfirmation(false); void close(); }} onCancel={() => setExitConfirmation(false)} />}
    {protonPreferencesOpen && <Suspense fallback={null}><ProtonPreferencesDialog overlayLayer={embedded ? 2300 : undefined} onClose={() => setProtonPreferencesOpen(false)} onBusyChange={setProtonPreferencesBusy} /></Suspense>}
  </main>;
}

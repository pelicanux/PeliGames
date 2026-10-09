import { useInstallerText } from "./installerText";
import { InstallerSettingsMenu } from "./InstallerSettingsMenu";
import { findPelinstallMatches, repairPeliGamesEntry, getPelinstallIcon, type ExecutableMatch } from "../../services/pelinstall";
import { fetchGameCoverByName } from "../../services/customCovers";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ModalSurface } from "../../components/ModalSurface";
import { BrandName } from "../../components/BrandName";
import { MenuIcon } from "../../components/MenuIcon";
import { ProtonSelector } from "../../components/ProtonSelector";
import { HoverTooltip } from "../../components/HoverTooltip";
import { ConfirmModal } from "../../components/ConfirmModal";
import { useInstallationLocation } from "../../hooks/useInstallationLocation";
import { openDirectoryPicker, openFilePicker } from "../../services/tauriService";
import { runGameInstaller, type GameInstallationResult } from "../../services/gameInstallation";
import { addPeliGamesEntry, listPeliGamesEntries, uninstallPeliGamesEntry } from "../../services/installedLibrary";
import type { GameInfo } from "../../components/GameGrid";
import type { StartupInfo } from "../startup/StartupRouter";
import "./pelinstall.css";
import { EntrySelector } from "./EntrySelector";
import { dragPelinstallWindow, PelinstallMinimizeButton } from "./WindowControls";

const preview = "__PELI_UI_PREVIEW__" in window;
export function Pelinstall({ startup }: { startup: StartupInfo }) {
  const tr = useInstallerText();
  const completionPreview = preview && new URLSearchParams(window.location.search).get("stage") === "complete";
  const sampleEntries: GameInfo[] = completionPreview ? ["7zFM", "7zG", "7z"].map(exe => ({ name: `7-Zip · ${exe}`, path: `/preview/${exe}.exe`, executable: `/preview/${exe}.exe`, launcher: "PeliGames", discovery_source: exe === "7zFM" ? "Atalho do Windows" : null })) : [];
  const panel = useRef<HTMLElement>(null);
  const [step, setStep] = useState(completionPreview ? 4 : -1);
  const [mode, setMode] = useState<"install" | "add" | "repair" | null>(completionPreview ? "install" : null);
  const [file, setFile] = useState(startup.error ? "" : startup.executable || "");
  const [matches, setMatches] = useState<ExecutableMatch[]>([]);
  const [checking, setChecking] = useState(Boolean(startup.executable && !startup.error));
  const [targetPath, setTargetPath] = useState("");
  const [repairExecutables, setRepairExecutables] = useState<GameInfo[]>([]);
  const [repairPrefix, setRepairPrefix] = useState("");
  const [uninstallConfirmation, setUninstallConfirmation] = useState(false);
  const [removed, setRemoved] = useState(false);
  const [executableIcon, setExecutableIcon] = useState("");
  const coverRequests = useRef(new Map<string, Promise<string | null>>());
  const target = matches.find(match => match.entry.path === targetPath) ?? matches[0];
  const [name, setName] = useState(completionPreview ? "7-Zip" : "");
  const [proton, setProton] = useState("");
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
  const [chosen, setChosen] = useState(sampleEntries[0]?.path || "");
  const [shortcut, setShortcut] = useState("");
  const [exitConfirmation, setExitConfirmation] = useState(false);
  const [cancelConfirmation, setCancelConfirmation] = useState(false);
  const [cancelPending, setCancelPending] = useState(false);
  const closeAfterCancel = useRef(false);
  const location = useInstallationLocation(name.trim(), mode !== null);
  const directory = mode === "repair" ? repairPrefix : location.directory;
  const directorySelected = mode === "repair" ? Boolean(repairPrefix.trim()) : location.selected;
  const validFile = Boolean(file && (mode !== "add" || file.toLowerCase().endsWith(".exe")));
  const stateRef = useRef({ busy, dirty: false, installing: false });
  const closing = useRef(false);
  stateRef.current = { busy, dirty: !result && Boolean(directorySelected || proton || name.trim()), installing: busy && step === 3 };
  const close = async () => {
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
  const cancelInstallation = async () => {
    setCancelPending(true); setCancelConfirmation(false); setError("");
    try { await invoke("cancel_game_installer"); }
    catch (error) { closeAfterCancel.current = false; setCancelPending(false); setError(String(error)); }
  };
  useEffect(() => {
    if (preview) return;
    let disposed = false; let remove: (() => void) | undefined;
    getCurrentWindow().onCloseRequested(event => {
      if (closing.current) return;
      if (stateRef.current.busy || stateRef.current.dirty) { event.preventDefault(); requestClose(); }
    }).then(unlisten => { if (disposed) unlisten(); else remove = unlisten; }).catch(error => setError(String(error)));
    return () => { disposed = true; remove?.(); };
  }, []);
  useEffect(() => {
    const openPreferences = () => { void invoke("open_peligames_launcher").catch(error => setError(String(error))); };
    window.addEventListener("openProtonPreferences", openPreferences);
    return () => window.removeEventListener("openProtonPreferences", openPreferences);
  }, []);
  useEffect(() => {
    if (!startup.executable || startup.error || (preview && startup.executable === "/preview/instalador.exe")) { setChecking(false); return; }
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
    setFile(repair ? target.entry.executable || startup.executable || "" : target.installer || startup.executable || ""); setStep(repair ? 1 : 0);
  };
  const uninstall = async () => {
    if (!target || busy) return;
    if (preview) { setUninstallConfirmation(false); setError("Execute o binário Pelinstall para desinstalar."); return; }
    setUninstallConfirmation(false); setBusy(true); setError("");
    try { await uninstallPeliGamesEntry(target.entry.path); setRemoved(true); setStep(5); setMatches([]); }
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
    try { const selected = await openDirectoryPicker(mode === "repair" ? "Selecionar prefixo Wine/Proton" : "Onde deseja instalar?"); if (selected) { if (mode === "repair") setRepairPrefix(selected); else location.setDirectory(selected); } }
    catch (error) { setError(String(error)); }
  };
  const install = async () => {
    if (busy || (mode === "repair" && (shortcutChecking || shortcutCheckFailed)) || !validFile || !directorySelected || !proton || !name.trim() || !mode) return;
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
      const cover = await resolveCover(name);
      const request = { name: name.trim(), directory, executable: file, proton, cover_url: cover || undefined };
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
      setEntries(registered); setChosen(registered.length === 1 ? registered[0].path : registered.find(entry => entry.discovery_source)?.path || "");
      if (installation.library_error) setError(installation.library_error);
      else if (!registered.length) setError("Nenhum executável foi encontrado. Abra o PeliGames para verificar a instalação e adicionar o executável.");
      setStep(4);
      if (mode === "repair" && !installation.library_error && removeExistingShortcuts) {
        setRemovedShortcutCount(await invoke<number>("remove_peligames_shortcuts", { path: targetPath })); setExistingShortcuts([]);
      }
      if (mode === "repair" && !installation.library_error && updateExistingShortcuts && !removeExistingShortcuts) {
        setUpdatedShortcutCount(await invoke<number>("update_peligames_shortcuts", { path: targetPath }));
      }
      if (!installation.library_error && desktop && registered.length === 1) setShortcut(await invoke<string>("create_peligames_shortcut", { path: registered[0].path }));
    } catch (error) { setError(String(error)); setStep(installed ? 4 : 2); }
    finally {
      setBusy(false); setCancelPending(false);
      if (closeAfterCancel.current) { closeAfterCancel.current = false; await close(); }
    }
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
    setError("");
    try { await invoke("open_peligames_launcher", { path: chosen || null }); stateRef.current.dirty = false; await close(); }
    catch (error) { setError(String(error)); }
  };
  useEffect(() => {
    if (preview || !panel.current) return;
    const scroller = panel.current.querySelector(".modal-scroll-body");
    if (!scroller) return;
    let frame = 0;
    const resize = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const children = Array.from(scroller.children).filter(child => getComputedStyle(child).display !== "none");
        const bottom = Math.max(...children.map(child => child.getBoundingClientRect().bottom));
        const height = Math.min(490, Math.max(260, Math.ceil(bottom - panel.current!.getBoundingClientRect().top + scroller.scrollTop + 24)));
        if (Math.abs(height - window.innerHeight) > 1) void getCurrentWindow().setSize(new LogicalSize(window.innerWidth, height)).catch(error => setError(String(error)));
      });
    };
    const observer = new ResizeObserver(resize);
    Array.from(scroller.children).forEach(child => observer.observe(child));
    resize();
    return () => { observer.disconnect(); cancelAnimationFrame(frame); };
  }, [step, error, result, shortcut, location.selected, checking, matches.length]);
  const stepActions = <footer className="pelinstall-actions">{step === 3 && <button disabled={cancelPending} onClick={() => { closeAfterCancel.current = false; setCancelConfirmation(true); }}>{cancelPending ? tr("Cancelando…") : mode === "repair" ? tr("Cancelar reparação") : tr("Cancelar instalação")}</button>}{step === 0 && <button disabled={busy} onClick={() => { setMode(null); setName(""); setProton(""); location.reset(); setRepairPrefix(""); setFile(startup.executable || ""); setError(""); setStep(-1); }}>{tr("Voltar")}</button>}{step > 0 && step < 3 && <button disabled={busy} onClick={() => { setError(""); if (mode === "repair" && step === 1) { setMode(null); setName(""); setProton(""); setRepairPrefix(""); location.reset(); setStep(-1); } else setStep(step-1); }}>{tr("Voltar")}</button>}{step >= 0 && step < 2 && <button className="primary" disabled={busy || (step === 0 ? !validFile || !directorySelected || !directory.trim() : !proton)} onClick={() => { setError(""); setStep(step+1); }}>{tr("Próximo")} <MenuIcon name="play" /></button>}{step === 2 && <button className="primary" disabled={busy || (mode === "repair" && (shortcutChecking || shortcutCheckFailed)) || !name.trim() || !validFile || !proton || !directory.trim()} onClick={() => void install()}>{busy ? mode === "add" ? tr("Adicionando…") : tr("Salvando…") : mode === "add" ? tr("Adicionar") : mode === "repair" ? tr("Reparar") : tr("Instalar")} <MenuIcon name={mode === "add" ? "folder" : "download"} /></button>}{(step === 4 || step === 5) && <><button disabled={busy} onClick={() => void close()}>{tr("Concluir")}</button><button className="primary" disabled={busy} onClick={() => void openLauncher()}>{tr("Abrir PeliGames")}</button></>}</footer>;
  const executableIdentity = <div className="pelinstall-identity"><div className="pelinstall-executable-badge">{executableIcon ? <img src={executableIcon} alt={tr("Ícone do executável")} /> : <MenuIcon name="document" />}<strong>{name.trim() || (step === -1 && target ? target.entry.name : file.split(/[\\/]/).pop()) || tr("Nenhum executável selecionado")}</strong></div></div>;
  return <main ref={panel} className="pelinstall-window">
    <ModalSurface className="modal-content dialog-glass pelinstall-surface" onDismiss={requestClose} closeDisabled={(busy && step !== 3) || cancelPending}
      onHeaderMouseDown={event => dragPelinstallWindow(event, setError)}
      headerActions={<><InstallerSettingsMenu /><button type="button" className="titlebar-title pelinstall-launcher-button" aria-label={tr("Abrir PeliGames")} title={tr("Abrir PeliGames")} disabled={launcherOpening} onClick={() => void openLauncherFromTitle()}><BrandName /></button><PelinstallMinimizeButton onError={setError} /></>}
      header={<h2><img src="/peligames.png" alt="" /><span className="pelinstall-brand">Pel<span className="titlebar-games">install</span></span></h2>}>
      {step !== 4 && <div className="pelinstall-step-layout"><div className="pelinstall-step-content">
      {step === -1 && checking && <p role="status">{tr("Consultando a biblioteca…")}</p>}
      {step === -1 && !checking && !matches.length && <section className="pelinstall-entry">
        <h3>{tr("Instalar ou adicionar?")}</h3>
        <div className="backend-options mod-manager-options">
          {(["install", "add"] as const).map(choice => <button key={choice} type="button" className="backend-option proton-preference-option mod-manager-option"
            onClick={() => { setMode(choice); setError(""); setStep(0); }}><MenuIcon name={choice === "install" ? "download" : "plusCircle"} /><span>{choice === "install" ? tr("Instalar") : tr("Adicionar")}</span></button>)}
        </div>
      </section>}
      {step === -1 && !checking && target && <section className="pelinstall-entry">
        <h3>{tr("Já está na biblioteca")}</h3>
        {matches.length > 1 && <EntrySelector label={tr("Registro na biblioteca")} value={target.entry.path} onChange={setTargetPath} options={matches.map((match,index) => ({ value: match.entry.path, label: `${index+1}. ${match.entry.name}` }))} />}
        <div className="backend-options mod-manager-options">
          <button className="backend-option proton-preference-option mod-manager-option" disabled={busy} onClick={() => beginExisting(false)}><MenuIcon name="plusCircle"/><span>{tr("Duplicar")}</span></button>
          <button className="backend-option proton-preference-option mod-manager-option" disabled={busy} onClick={() => beginExisting(true)}><MenuIcon name="repair"/><span>{tr("Reparar")}</span></button>
          <button className="backend-option proton-preference-option mod-manager-option" disabled={busy} onClick={() => setUninstallConfirmation(true)}><MenuIcon name="trash"/><span>{tr("Desinstalar")}</span></button>
          <button className="backend-option proton-preference-option mod-manager-option" disabled={busy} onClick={() => void openSettings()}><MenuIcon name="settings"/><span>{tr("Configurações")}</span></button>
        </div>
      </section>}
      {step === 0 && <section>
        <h3>{mode === "repair" ? tr("Qual prefixo deseja usar para reparar?") : mode === "add" ? tr("Onde deseja criar o prefixo?") : tr("Onde deseja instalar?")}</h3>
        {!validFile && <button className="installation-name-confirm" onClick={() => void chooseFile()}><MenuIcon name="folderSearch" />{mode === "repair" ? tr("Selecionar instalador") : tr("Selecionar executável")}</button>}
        {!directorySelected ? <div className="pelinstall-two-buttons"><button onClick={() => void location.selectDefault().catch(error => setError(String(error)))}>{tr("Padrão")}</button><button onClick={() => void chooseDirectory()}>{tr("Escolher")}</button></div> : <HoverTooltip text={directory} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip"><div className="game-directory-value installation-path-box"><input aria-label={tr("Local de instalação")} className="installation-path-input" value={directory} onChange={event => { if (mode === "repair") setRepairPrefix(event.target.value); else location.setDirectory(event.target.value); }} /><button className="installation-path-picker" aria-label={tr("Alterar local de instalação")} onClick={() => void chooseDirectory()}><MenuIcon name="folderSearch" /></button></div></HoverTooltip>}
      </section>}
      {step === 1 && <section><h3>{mode === "add" ? tr("Executar com Proton") : mode === "repair" ? tr("Reparar com Proton") : tr("Instalar com Proton")}</h3><ProtonSelector value={proton} onChange={setProton} label={tr("Versão do Proton")} disabled={busy} /></section>}
      {step === 2 && <section><h3>{tr("Nome")}</h3><div className="game-directory-value installation-path-box"><input aria-label={tr("Nome")} className="installation-path-input" placeholder={tr("Digite o nome")} value={name} onChange={event => setName(event.target.value)} disabled={busy} /></div>{mode === "repair" && <><label className="installation-field-label">{tr("Executável")}</label><EntrySelector label={tr("Executável para reparar")} value={file} onChange={setFile} disabled={busy} options={Array.from(new Map([...(target ? [target.entry] : []), ...repairExecutables].map(entry => [entry.executable || "", { value: entry.executable || "", label: entry.executable?.split(/[\\/]/).pop() || entry.name }])).values()).filter(option => option.value)} /></>}<label className="pelinstall-desktop"><input type="checkbox" checked={mode === "repair" && existingShortcuts.length > 0 ? removeExistingShortcuts : desktop} onChange={event => { if (mode === "repair" && existingShortcuts.length > 0) { setRemoveExistingShortcuts(event.target.checked); if (event.target.checked) setUpdateExistingShortcuts(false); } else setDesktop(event.target.checked); }} disabled={busy || (mode === "repair" && (shortcutChecking || shortcutCheckFailed))} />{mode === "repair" && shortcutChecking ? tr("Verificando atalhos…") : mode === "repair" && shortcutCheckFailed ? tr("Não foi possível verificar os atalhos") : mode === "repair" && existingShortcuts.length > 0 ? tr("Remover atalho da área de trabalho") : tr("Criar atalho na área de trabalho")}</label>{mode === "repair" && existingShortcuts.length > 0 && <button className={`pelinstall-update-shortcut ${updateExistingShortcuts ? "active" : ""}`} aria-pressed={updateExistingShortcuts} disabled={busy || shortcutChecking || shortcutCheckFailed} onClick={() => { setUpdateExistingShortcuts(!updateExistingShortcuts); setRemoveExistingShortcuts(false); }}><MenuIcon name="repair" />{updateExistingShortcuts ? tr("Atualizar atalho ao reparar ✓") : tr("Atualizar atalho")}</button>}</section>}
      {step === 3 && <section role="status"><h3>{mode === "repair" ? tr("Reparação em andamento") : tr("Instalação em andamento")}</h3><p>{cancelPending ? tr("Encerrando os processos…") : mode === "repair" ? tr("Aguardando a atualização do prefixo…") : tr("Aguardando o instalador Windows finalizar…")}</p><p className="pelinstall-note">{mode === "repair" ? tr("A conclusão será liberada quando o Proton terminar de preparar o prefixo.") : tr("O Pelinstall permanece aberto enquanto o Proton prepara o prefixo e o instalador está em execução.")}</p></section>}
      {removed && step === 5 && <section><h3>{tr("Prefixo desinstalado")}</h3><p>{tr("Os registros que utilizavam esse prefixo foram removidos da biblioteca.")}</p></section>}
      {stepActions}
      </div>{executableIdentity}</div>}
      {step === 4 && <section className="pelinstall-completion">
        <div className="pelinstall-completion-header">
          <div className="pelinstall-completion-summary">
            <h3>{mode === "add" ? tr("Adicionado à biblioteca") : mode === "repair" ? tr("Reparação finalizada") : tr("Instalação finalizada")}</h3>
            <p>{entries.length} {tr("executável(is) registrado(s) no PeliGames.")}</p>
          </div>
          {executableIdentity}
        </div>
        {entries.length > 1 && <div className="pelinstall-completion-executables">
          <label className="installation-field-label" htmlFor="shortcut-executable">{tr("Qual executável deseja usar?")}</label>
          <EntrySelector label={tr("Qual executável deseja usar?")} value={chosen} onChange={setChosen} disabled={busy} options={entries.map(entry => ({ value: entry.path, label: `${entry.name} — ${entry.executable?.split(/[\\/]/).pop()}${entry.discovery_source ? ` (${entry.discovery_source})` : ""}` }))} />
        </div>}
        {entries.length > 0 && <div className="pelinstall-completion-buttons">
          {desktop && !shortcut && entries.length > 1 && <button disabled={!chosen || busy} onClick={() => void createShortcut()}>{tr("Criar atalho")}</button>}
          <button className="primary" disabled={!chosen || busy} onClick={() => { if (preview) { setError("Execute o binário Pelinstall para iniciar o programa."); return; } setBusy(true); setError(""); void invoke("launch_pelinstall_entry", { path: chosen }).then(() => close()).catch(error => setError(String(error))).finally(() => setBusy(false)); }}>{tr("Iniciar")} <MenuIcon name="play" /></button>
        </div>}
        {updatedShortcutCount > 0 && <p className="pelinstall-note">{tr("Atalho atualizado.")}</p>}
        {removedShortcutCount > 0 && <p className="pelinstall-note">{removedShortcutCount === 1 ? tr("Atalho removido.") : tr("Atalhos removidos.")}</p>}
        {shortcut && <p className="pelinstall-note">{tr("Atalho criado:")} {shortcut}</p>}
        {result?.log && <p className="pelinstall-note">Log: {result.log}</p>}
      </section>}

      {step === 2 && <div className="pelinstall-step-notes"><HoverTooltip text={directory}><p className="pelinstall-note">{tr("Destino:")} {directory}</p></HoverTooltip><p className="pelinstall-note">{mode === "add" ? tr("O executável será adicionado à biblioteca com o prefixo e o Proton selecionados.") : mode === "repair" ? tr("O prefixo será atualizado se o Proton escolhido for diferente do atual.") : tr("O instalador Windows abrirá suas próprias telas. Finalize a instalação nele para registrar os executáveis na biblioteca.")}</p></div>}
      {error && <p className="pelinstall-error" role="alert">{error}</p>}
      {step === 4 && stepActions}
    </ModalSurface>
    {uninstallConfirmation && target && <ConfirmModal title={tr("Desinstalar?")} message={`${tr("Todo o prefixo será apagado permanentemente, incluindo jogos, programas, configurações e saves. Todos os jogos que usam esse prefixo serão removidos da biblioteca. Os atalhos também serão removidos da área de trabalho e das pastas de aplicativos. Outros arquivos fora do prefixo serão preservados. Deseja continuar?")}\n${target.entry.name}\n${target.entry.prefix}`} confirmText={tr("Desinstalar")} onConfirm={() => void uninstall()} onCancel={() => setUninstallConfirmation(false)} />}
    {cancelConfirmation && <ConfirmModal title={mode === "repair" ? tr("Cancelar reparação?") : tr("Cancelar instalação?")} message={mode === "repair" ? tr("Deseja encerrar a atualização do prefixo? Os arquivos e logs serão preservados, mas a preparação pode ficar incompleta. As alterações do registro não serão salvas.") : tr("Deseja encerrar o instalador e os processos deste prefixo? A instalação pode ficar incompleta. O prefixo e os logs serão preservados e nenhum jogo será registrado por esta tentativa.")} confirmText={tr("Encerrar instalação")} onConfirm={() => void cancelInstallation()} onCancel={() => { closeAfterCancel.current = false; setCancelConfirmation(false); }} />}
    {exitConfirmation && <ConfirmModal title={tr("Configuração incompleta")} message={tr("Deseja sair e descartar a configuração iniciada?")} onConfirm={() => { stateRef.current.dirty = false; setExitConfirmation(false); void close(); }} onCancel={() => setExitConfirmation(false)} />}
  </main>;
}

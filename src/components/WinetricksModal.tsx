import { LogText } from "./LogText";
import { TimedFeedback } from "./TimedFeedback";
import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { runWineTool, cancelWineTool, isWineToolCancelled, type WinePackage } from "../services/wineTools";
import { indexWineCatalog, searchWineCatalog, wineCatalogQuery, WINE_RESULTS_PAGE, type WineCatalogFilter } from "../services/winetricksCatalog";
import { refreshWineCatalogAfter } from "../services/wineToolRefresh";
import { wineToolProgress, type WineToolProgress } from "../services/wineToolProgress";
import "./WinetricksModal.css";

/** Winetricks catalog of the game prefix: search, pick and install components with the
 * Proton already chosen for the game (it is chosen in the Execution options, not here). */
export function WinetricksModal({ path, name, proton, onClose }: { path: string; name: string; proton: string; onClose: () => void }) {
  const { language } = useI18n(), pt = language !== "en";
  const titleId = useId(); const terminal = useRef<HTMLPreElement>(null); const started = useRef(false);
  const [packages, setPackages] = useState<WinePackage[]>([]), [selection, setSelection] = useState<string[]>([]), [search, setSearch] = useState(""), [filter, setFilter] = useState<WineCatalogFilter>("all");
  const [visibleCount, setVisibleCount] = useState(WINE_RESULTS_PAGE);
  const [busy, setBusy] = useState(true), [error, setError] = useState(""), [lines, setLines] = useState<string[]>([]), [complete, setComplete] = useState("");
  const [progress, setProgress] = useState<WineToolProgress | null>({ phase: "preparing" });
  const [operation, setOperation] = useState<"catalog" | "install" | "interface">("catalog");
  const activeRequest = useRef<string | null>(null);
  const cancelling = useRef(false);
  const [cancelReady, setCancelReady] = useState(false);
  const [cancelPending, setCancelPending] = useState(false);
  const [externalTool, setExternalTool] = useState<"winetricks-gui" | "uninstaller">("winetricks-gui");
  const [installing, setInstalling] = useState<string[]>([]);
  const append = useCallback((line: string) => {
    setLines(previous => [...previous, line].slice(-200));
    if (activeRequest.current && !cancelling.current) setCancelReady(true);
    setProgress(previous => previous ? wineToolProgress(previous, line) : previous);
  }, []);
  useEffect(() => {
    if (progress?.phase !== "complete") return;
    const timer = window.setTimeout(() => setProgress(null), 3000);
    return () => window.clearTimeout(timer);
  }, [progress?.phase]);
  useEffect(() => { if (terminal.current) terminal.current.scrollTop = terminal.current.scrollHeight; }, [lines]);
  const runTool = useCallback(async (action: Parameters<typeof runWineTool>[1], ids: string[] = []) => {
    const id = crypto.randomUUID(); activeRequest.current = id;
    setCancelReady(false);
    try { return await runWineTool(path, action, ids, append, id); }
    finally { if (activeRequest.current === id) {activeRequest.current = null; setCancelReady(false);} }
  }, [path, append]);
  const load = useCallback(async () => {
    const result = await runTool("catalog");
    setPackages(result.packages);
    setSelection(previous => previous.filter(id => !result.packages.some(item => item.id === id && item.installed)));
    return result.packages;
  }, [runTool]);
  useEffect(() => { if (started.current) return; started.current = true; if (!proton) { setBusy(false); setProgress(null); return; } void load().then(() => setProgress(null)).catch(e => { if (isWineToolCancelled(e)) setProgress({phase: "cancelled"}); else {setError(String(e)); setProgress({phase: "failed"});} }).finally(() => {setBusy(false); setCancelPending(false); cancelling.current=false;}); }, [load, proton]);
  useEffect(() => {
    const previous = document.activeElement;
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopImmediatePropagation(); if (!busy) onClose(); } };
    window.addEventListener("keydown", key, true); return () => { window.removeEventListener("keydown", key, true); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [busy, onClose]);
  const step = async (kind: "catalog" | "install" | "interface", action: () => Promise<unknown>) => {
    if (busy) return;
    cancelling.current=false; setCancelPending(false);
    setBusy(true); setOperation(kind); setError(""); setComplete(""); setLines([]);
    setProgress({phase: kind === "interface" ? "interface" : "preparing"});
    try { await action(); setProgress(kind === "catalog" ? null : {phase: "complete"}); }
    catch (failure) {
      if (isWineToolCancelled(failure)) {setProgress({phase: "cancelled"}); setError("");}
      else {setError(String(failure)); setProgress({phase: "failed"});}
    }
    finally { setBusy(false); setCancelPending(false); cancelling.current=false; }
  };
  const install = () => { setInstalling([...selection]); void step("install", async () => {
    const result = await refreshWineCatalogAfter(
      () => runTool("install", selection),
      () => { setProgress({phase: "verifying"}); return load(); },
    );
    setFilter("installed"); setSearch(""); setVisibleCount(WINE_RESULTS_PAGE);
    setComplete(result.log); setSelection([]);
  }); };
  const openInterface = (tool: "winetricks-gui" | "uninstaller") => {
    if (busy || !proton) return;
    setExternalTool(tool);
    void step("interface", async () => {
      await refreshWineCatalogAfter(
        () => runTool(tool),
        () => { setProgress({phase: "verifying"}); return load(); },
      );
    });
  };
  const cancelOperation = async () => {
    const id = activeRequest.current;
    if (!id || cancelling.current) return;
    const previous = progress;
    cancelling.current=true; setCancelPending(true); setCancelReady(false); setProgress({phase: "cancelling"});
    try { await cancelWineTool(id); }
    catch (failure) {
      // A completed operation must not be replaced by a late cancellation response.
      if (activeRequest.current === id) {
        cancelling.current=false; setCancelPending(false); setCancelReady(true); setProgress(previous); setError(String(failure));
      }
    }
  };
  const catalog = useMemo(() => {
    const indexed = indexWineCatalog(packages);
    const counts = { all: packages.length, dlls: 0, fonts: 0, installed: 0 };
    for (const item of packages) { if (item.category === "fonts") counts.fonts++; else counts.dlls++; if (item.installed) counts.installed++; }
    return { indexed, counts };
  }, [packages]);
  const query = wineCatalogQuery(search);
  const filtered = useMemo(() => searchWineCatalog(catalog.indexed, query, filter), [catalog, query, filter]);
  const filters: [WineCatalogFilter, string, number][] = [
    ["all", pt ? "Todos" : "All", catalog.counts.all],
    ["dlls", pt ? "DLLs e componentes" : "DLLs and components", catalog.counts.dlls],
    ["fonts", pt ? "Fontes" : "Fonts", catalog.counts.fonts],
    ["installed", pt ? "Instalados" : "Installed", catalog.counts.installed],
  ];
  const toggle = (id: string, on: boolean) => setSelection(previous => on ? [...previous, id] : previous.filter(item => item !== id));
  const protonName = proton.split("/").filter(Boolean).pop() || proton;
  const phases = pt ? {preparing: operation === "catalog" ? "Carregando componentes" : "Preparando instalação", downloading: "Baixando arquivo", installing: "Instalando componentes", verifying: "Atualizando componentes instalados", interface: externalTool === "uninstaller" ? "Desinstalador aberto" : "Winetricks aberto", complete: "Operação concluída", failed: "Não foi possível concluir", cancelling: "Cancelando operação", cancelled: "Operação cancelada"}
    : {preparing: operation === "catalog" ? "Loading components" : "Preparing installation", downloading: "Downloading file", installing: "Installing components", verifying: "Refreshing installed components", interface: externalTool === "uninstaller" ? "Uninstaller is open" : "Winetricks is open", complete: "Operation complete", failed: "Unable to finish", cancelling: "Cancelling operation", cancelled: "Operation cancelled"};
  const phaseHint = progress?.phase === "cancelling" ? (pt ? "Encerrando os processos desta operação. Aguarde…" : "Stopping this operation's processes. Please wait…")
    : progress?.phase === "cancelled" ? (pt ? "A execução foi interrompida. Alterações já feitas não são desfeitas; o componente pode precisar de reparação." : "Execution was interrupted. Existing changes were not reverted; the component may need repair.")
    : progress?.phase === "interface" ? (externalTool === "uninstaller" ? (pt ? "Selecione o componente na janela aberta e escolha Remover. Feche-a para continuar." : "Select the component in the opened window and choose Remove. Close it to continue.") : (pt ? "Conclua a operação na janela do Winetricks para continuar." : "Finish the operation in the Winetricks window to continue."))
    : progress?.phase === "installing" ? (pt ? "Alguns instaladores abrem uma janela própria. Siga as instruções nela." : "Some installers open their own window. Follow its instructions.")
    : progress?.phase === "downloading" ? (pt ? "O percentual corresponde somente ao arquivo em download." : "The percentage refers only to the current download.")
    : progress?.phase === "complete" ? (pt ? "Os componentes deste prefixo foram atualizados." : "This prefix's component list has been refreshed.")
    : progress?.phase === "failed" ? (pt ? "Consulte o registro técnico abaixo antes de tentar novamente." : "Check the technical log below before retrying.")
    : (pt ? "Acompanhe a operação aqui; os detalhes ficam no registro técnico." : "Follow the operation here; details are in the technical log.");
  return createPortal(<div className="modal-overlay" onClick={e => { if (e.target === e.currentTarget && !busy) onClose(); }}>
    <ModalSurface className="modal-content dialog-glass winetricks-modal" role="dialog" aria-modal="true" aria-labelledby={titleId} onDismiss={() => { if (!busy) onClose(); }} closeDisabled={busy}
      header={<div className="winetricks-title"><span className="winetricks-title-icon"><MenuIcon name="cube" /></span><h2 id={titleId}><span>Winetricks</span><span className="winetricks-title-separator" aria-hidden="true">·</span><span className="winetricks-title-game" title={name}>{name}</span></h2></div>}
      footer={<div className="winetricks-bottom">
      {progress && <section className={`winetricks-progress${progress.phase === "failed" ? " is-error" : ""}`} aria-label={pt ? "Andamento do Winetricks" : "Winetricks progress"}>
        <div className="winetricks-progress-heading"><span className="winetricks-section-label">{["failed", "cancelled", "complete"].includes(progress.phase) ? (pt ? "Resultado" : "Result") : (pt ? "Andamento" : "Progress")}</span><span>{progress.phase === "downloading" && progress.percent !== undefined ? `${progress.percent}%` : busy ? (pt ? "Em andamento" : "In progress") : ""}</span></div>
        <div className="winetricks-progress-status" role="status" aria-live="polite"><strong>{phases[progress.phase]}</strong>{progress.component && <span className="winetricks-component">{progress.component}</span>}</div>
        {operation === "install" && installing.length > 0 && <p className="winetricks-progress-selection" title={installing.join(", ")}>{installing.join(" · ")}</p>}
        <p>{phaseHint}</p>
        {progress.file && <small className="winetricks-progress-file" title={progress.file}>{progress.file.split(/[\\/]/).pop()}{progress.speed && ` · ${progress.speed}`}</small>}
        {!["failed", "cancelled"].includes(progress.phase) && <div className={`winetricks-progress-track${busy && !(progress.phase === "downloading" && progress.percent !== undefined) ? " is-indeterminate" : ""}`} role="progressbar" aria-label={phases[progress.phase]} aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress.phase === "complete" ? 100 : progress.phase === "downloading" ? progress.percent : undefined}><i style={{width: progress.phase === "complete" ? "100%" : progress.phase === "downloading" && progress.percent !== undefined ? `${progress.percent}%` : undefined}} /></div>}
      </section>}
      <details className="winetricks-log" open={Boolean(error) || undefined}>
        <summary>{pt ? "Registro técnico" : "Technical log"}</summary>
        <pre ref={terminal} className="wine-tool-terminal game-log-output" aria-label={pt ? "Log do Winetricks" : "Winetricks log"} role="log"><LogText content={lines.join("\n") || (pt ? "Preparando Winetricks…" : "Preparing Winetricks…")}/></pre>
      </details>
      {error && <TimedFeedback role="alert" className="advanced-validation-error">{error}</TimedFeedback>}{complete && <TimedFeedback><span title={complete}>{pt ? "Componentes instalados. O registro foi salvo no disco." : "Components installed. The log was saved to disk."}</span></TimedFeedback>}
      <div className="winetricks-actions">
        <span role="status">{busy && progress ? phases[progress.phase] : pt ? `${selection.length} selecionado(s)` : `${selection.length} selected`}</span>
        {(error || progress?.phase === "cancelled") && !packages.length && !busy && proton && <button type="button" className="modern-mini-btn" onClick={() => void step("catalog", load)}><MenuIcon name="repair" />{pt ? "Tentar novamente" : "Try again"}</button>}
        {busy && <button type="button" className="modern-mini-btn" disabled={!cancelReady || cancelPending} onClick={() => void cancelOperation()} title={pt ? "Interrompe esta operação. Alterações já feitas no prefixo não são desfeitas automaticamente." : "Stops this operation. Changes already made to the prefix are not automatically reverted."}><MenuIcon name="stop" />{cancelPending ? (pt ? "Cancelando…" : "Cancelling…") : (pt ? "Cancelar" : "Cancel")}</button>}
        <button type="button" className="modern-mini-btn winetricks-install" disabled={busy || !proton || !selection.length} onClick={install}><MenuIcon name="download" />{pt ? "Instalar selecionados" : "Install selected"}</button>
      </div></div>}>
      <div className="winetricks-section-label">{pt ? "Componentes do jogo" : "Game components"}</div>
      <div className="winetricks-toolbar">
        <p className="winetricks-intro">{pt ? "Instale DLLs, fontes e dependências no prefixo deste jogo." : "Install DLLs, fonts and dependencies in this game's prefix."}{proton && <span className="winetricks-proton" title={proton}><MenuIcon name="cube" />{protonName}</span>}</p>
        <div className="winetricks-tools"><button type="button" className="modern-mini-btn" disabled={busy || !proton} title={pt ? "Abre o Winetricks no prefixo deste jogo. Feche a janela dele para continuar aqui." : "Opens Winetricks in this game's prefix. Close its window to continue here."} onClick={() => openInterface("winetricks-gui")}><MenuIcon name="externalLink" />{pt ? "Abrir Winetricks" : "Open Winetricks"}</button>
        <button type="button" className="modern-mini-btn" disabled={busy || !proton} onClick={() => openInterface("uninstaller")} title={pt ? "Abre o desinstalador do Wine neste prefixo. Selecione o componente, como Microsoft .NET, e use Remover. Nem todo componente registra um desinstalador." : "Opens Wine's uninstaller in this prefix. Select a component, such as Microsoft .NET, and use Remove. Not every component registers an uninstaller."}><MenuIcon name="trash" />{pt ? "Desinstalar componentes" : "Uninstall components"}</button></div>
      </div>
      <label className="winetricks-search"><MenuIcon name="search" /><input autoFocus type="search" aria-label={pt ? "Buscar componentes Winetricks" : "Search Winetricks components"} placeholder={pt ? "Buscar DLLs, fontes ou dependências…" : "Search DLLs, fonts or dependencies…"} value={search} onChange={e => { setSearch(e.target.value); setVisibleCount(WINE_RESULTS_PAGE); }} /></label>
      <div className="winetricks-filters" role="group" aria-label={pt ? "Filtrar componentes" : "Filter components"}>
        {filters.map(([id, label, count]) => <button type="button" key={id} aria-pressed={filter === id} onClick={() => { setFilter(id); setVisibleCount(WINE_RESULTS_PAGE); }}>{label}<span>{count}</span></button>)}
      </div>
      <fieldset disabled={busy} className="winetricks-packages" aria-busy={busy}>
        {filtered.slice(0, visibleCount).map(p => <label key={p.id} className="winetricks-package" data-installed={p.installed || undefined}>
          <input type="checkbox" checked={p.installed || selection.includes(p.id)} disabled={p.installed} onChange={e => toggle(p.id, e.target.checked)} />
          <span><strong>{p.id}</strong><small>{p.title}</small></span>
          <em>{p.installed ? (pt ? "Instalado" : "Installed") : p.category === "fonts" ? (pt ? "Fonte" : "Font") : (pt ? "DLL / componente" : "DLL / component")}</em>
        </label>)}
        {!filtered.length && <p className="winetricks-empty" role="status">{busy && progress ? phases[progress.phase] : !proton ? (pt ? "Escolha o Proton do jogo em Opções → Execução para ver os componentes." : "Choose the game's Proton in Options → Execution to see the components.") : !query && filter !== "installed" ? (pt ? "Digite o nome de uma DLL, fonte ou dependência para buscar." : "Enter a DLL, font or dependency name to search.") : filter === "installed" && !query ? (pt ? "Nenhum componente instalado neste prefixo." : "No components installed in this prefix.") : (pt ? "Nenhum componente encontrado." : "No components found.")}</p>}
        {filtered.length > visibleCount && <button type="button" className="modern-mini-btn" onClick={() => setVisibleCount(count => count + WINE_RESULTS_PAGE)}>{pt ? `Mostrar mais resultados (${Math.min(visibleCount, filtered.length)} de ${filtered.length})` : `Show more results (${Math.min(visibleCount, filtered.length)} of ${filtered.length})`}</button>}
      </fieldset>
    </ModalSurface>
  </div>, document.body);
}

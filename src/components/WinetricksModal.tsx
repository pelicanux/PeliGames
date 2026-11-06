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
  const {t: tx} = useI18n();

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
    ["all", tx("ui-winetricks-modal", "all"), catalog.counts.all],
    ["dlls", tx("ui-winetricks-modal", "dlls_and_components"), catalog.counts.dlls],
    ["fonts", tx("ui-winetricks-modal", "fonts"), catalog.counts.fonts],
    ["installed", tx("ui-winetricks-modal", "installed"), catalog.counts.installed],
  ];
  const toggle = (id: string, on: boolean) => setSelection(previous => on ? [...previous, id] : previous.filter(item => item !== id));
  const protonName = proton.split("/").filter(Boolean).pop() || proton;
  const phases = {preparing: (operation === "catalog" ? tx("ui-winetricks-modal", "loading_components") : tx("ui-winetricks-modal", "preparing_installation")), downloading: tx("ui-winetricks-modal", "downloading_file"), installing: tx("ui-winetricks-modal", "installing_components"), verifying: tx("ui-winetricks-modal", "refreshing_installed_components"), interface: (externalTool === "uninstaller" ? tx("ui-winetricks-modal", "uninstaller_is_open") : tx("ui-winetricks-modal", "winetricks_is_open")), complete: tx("ui-winetricks-modal", "operation_complete"), failed: tx("ui-winetricks-modal", "unable_to_finish"), cancelling: tx("ui-winetricks-modal", "cancelling_operation"), cancelled: tx("ui-winetricks-modal", "operation_cancelled")};
  const phaseHint = progress?.phase === "cancelling" ? (tx("ui-winetricks-modal", "stopping_this_operation_s_processes_please_wait"))
    : progress?.phase === "cancelled" ? (tx("ui-winetricks-modal", "execution_was_interrupted_existing_changes_were_not_reverted_the_component_may_need_repa"))
    : progress?.phase === "interface" ? (externalTool === "uninstaller" ? (tx("ui-winetricks-modal", "select_the_component_in_the_opened_window_and_choose_remove_close_it_to_continue")) : (tx("ui-winetricks-modal", "finish_the_operation_in_the_winetricks_window_to_continue")))
    : progress?.phase === "installing" ? (tx("ui-winetricks-modal", "some_installers_open_their_own_window_follow_its_instructions"))
    : progress?.phase === "downloading" ? (tx("ui-winetricks-modal", "the_percentage_refers_only_to_the_current_download"))
    : progress?.phase === "complete" ? (tx("ui-winetricks-modal", "this_prefix_s_component_list_has_been_refreshed"))
    : progress?.phase === "failed" ? (tx("ui-winetricks-modal", "check_the_technical_log_below_before_retrying"))
    : (tx("ui-winetricks-modal", "follow_the_operation_here_details_are_in_the_technical_log"));
  return createPortal(<div className="modal-overlay" onClick={e => { if (e.target === e.currentTarget && !busy) onClose(); }}>
    <ModalSurface className="modal-content dialog-glass winetricks-modal" role="dialog" aria-modal="true" aria-labelledby={titleId} onDismiss={() => { if (!busy) onClose(); }} closeDisabled={busy}
      header={<div className="winetricks-title"><span className="winetricks-title-icon"><MenuIcon name="cube" /></span><h2 id={titleId}><span>Winetricks</span><span className="winetricks-title-separator" aria-hidden="true">·</span><span className="winetricks-title-game" title={name}>{name}</span></h2></div>}
      footer={<div className="winetricks-bottom">
      {progress && <section className={`winetricks-progress${progress.phase === "failed" ? " is-error" : ""}`} aria-label={tx("ui-winetricks-modal", "winetricks_progress")}>
        <div className="winetricks-progress-heading"><span className="winetricks-section-label">{["failed", "cancelled", "complete"].includes(progress.phase) ? (tx("ui-winetricks-modal", "result")) : (tx("ui-winetricks-modal", "progress"))}</span><span>{progress.phase === "downloading" && progress.percent !== undefined ? `${progress.percent}%` : busy ? (tx("ui-winetricks-modal", "in_progress")) : ""}</span></div>
        <div className="winetricks-progress-status" role="status" aria-live="polite"><strong>{phases[progress.phase]}</strong>{progress.component && <span className="winetricks-component">{progress.component}</span>}</div>
        {operation === "install" && installing.length > 0 && <p className="winetricks-progress-selection" title={installing.join(", ")}>{installing.join(" · ")}</p>}
        <p>{phaseHint}</p>
        {progress.file && <small className="winetricks-progress-file" title={progress.file}>{progress.file.split(/[\\/]/).pop()}{progress.speed && ` · ${progress.speed}`}</small>}
        {!["failed", "cancelled"].includes(progress.phase) && <div className={`winetricks-progress-track${busy && !(progress.phase === "downloading" && progress.percent !== undefined) ? " is-indeterminate" : ""}`} role="progressbar" aria-label={phases[progress.phase]} aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress.phase === "complete" ? 100 : progress.phase === "downloading" ? progress.percent : undefined}><i style={{width: progress.phase === "complete" ? "100%" : progress.phase === "downloading" && progress.percent !== undefined ? `${progress.percent}%` : undefined}} /></div>}
      </section>}
      <details className="winetricks-log" open={Boolean(error) || undefined}>
        <summary>{tx("ui-winetricks-modal", "technical_log")}</summary>
        <pre ref={terminal} className="wine-tool-terminal game-log-output" aria-label={tx("ui-winetricks-modal", "winetricks_log")} role="log"><LogText content={lines.join("\n") || (tx("ui-winetricks-modal", "preparing_winetricks"))}/></pre>
      </details>
      {error && <TimedFeedback role="alert" className="advanced-validation-error">{error}</TimedFeedback>}{complete && <TimedFeedback><span title={complete}>{tx("ui-winetricks-modal", "components_installed_the_log_was_saved_to_disk")}</span></TimedFeedback>}
      <div className="winetricks-actions">
        <span role="status">{busy && progress ? phases[progress.phase] : tx("ui-winetricks-modal", "selected", {"selection_length": selection.length})}</span>
        {(error || progress?.phase === "cancelled") && !packages.length && !busy && proton && <button type="button" className="modern-mini-btn" onClick={() => void step("catalog", load)}><MenuIcon name="repair" />{tx("ui-winetricks-modal", "try_again")}</button>}
        {busy && <button type="button" className="modern-mini-btn" disabled={!cancelReady || cancelPending} onClick={() => void cancelOperation()} title={tx("ui-winetricks-modal", "stops_this_operation_changes_already_made_to_the_prefix_are_not_automatically_reverted")}><MenuIcon name="stop" />{cancelPending ? (tx("ui-winetricks-modal", "cancelling")) : (tx("ui-winetricks-modal", "cancel"))}</button>}
        <button type="button" className="modern-mini-btn winetricks-install" disabled={busy || !proton || !selection.length} onClick={install}><MenuIcon name="download" />{tx("ui-winetricks-modal", "install_selected")}</button>
      </div></div>}>
      <div className="winetricks-section-label">{tx("ui-winetricks-modal", "game_components")}</div>
      <div className="winetricks-toolbar">
        <p className="winetricks-intro">{tx("ui-winetricks-modal", "install_dlls_fonts_and_dependencies_in_this_game_s_prefix")}{proton && <span className="winetricks-proton" title={proton}><MenuIcon name="cube" />{protonName}</span>}</p>
        <div className="winetricks-tools"><button type="button" className="modern-mini-btn" disabled={busy || !proton} title={tx("ui-winetricks-modal", "opens_winetricks_in_this_game_s_prefix_close_its_window_to_continue_here")} onClick={() => openInterface("winetricks-gui")}><MenuIcon name="externalLink" />{tx("ui-winetricks-modal", "open_winetricks")}</button>
        <button type="button" className="modern-mini-btn" disabled={busy || !proton} onClick={() => openInterface("uninstaller")} title={tx("ui-winetricks-modal", "opens_wine_s_uninstaller_in_this_prefix_select_a_component_such_as_microsoft_net_and_use")}><MenuIcon name="trash" />{tx("ui-winetricks-modal", "uninstall_components")}</button></div>
      </div>
      <label className="winetricks-search"><MenuIcon name="search" /><input autoFocus type="search" aria-label={tx("ui-winetricks-modal", "search_winetricks_components")} placeholder={tx("ui-winetricks-modal", "search_dlls_fonts_or_dependencies")} value={search} onChange={e => { setSearch(e.target.value); setVisibleCount(WINE_RESULTS_PAGE); }} /></label>
      <div className="winetricks-filters" role="group" aria-label={tx("ui-winetricks-modal", "filter_components")}>
        {filters.map(([id, label, count]) => <button type="button" key={id} aria-pressed={filter === id} onClick={() => { setFilter(id); setVisibleCount(WINE_RESULTS_PAGE); }}>{label}<span>{count}</span></button>)}
      </div>
      <fieldset disabled={busy} className="winetricks-packages" aria-busy={busy}>
        {filtered.slice(0, visibleCount).map(p => <label key={p.id} className="winetricks-package" data-installed={p.installed || undefined}>
          <input type="checkbox" checked={p.installed || selection.includes(p.id)} disabled={p.installed} onChange={e => toggle(p.id, e.target.checked)} />
          <span><strong>{p.id}</strong><small>{p.title}</small></span>
          <em>{p.installed ? (tx("ui-winetricks-modal", "installed_2")) : p.category === "fonts" ? (tx("ui-winetricks-modal", "font")) : (tx("ui-winetricks-modal", "dll_component"))}</em>
        </label>)}
        {!filtered.length && <p className="winetricks-empty" role="status">{busy && progress ? phases[progress.phase] : !proton ? (tx("ui-winetricks-modal", "choose_the_game_s_proton_in_options_execution_to_see_the_components")) : !query && filter !== "installed" ? (tx("ui-winetricks-modal", "enter_a_dll_font_or_dependency_name_to_search")) : filter === "installed" && !query ? (tx("ui-winetricks-modal", "no_components_installed_in_this_prefix")) : (tx("ui-winetricks-modal", "no_components_found"))}</p>}
        {filtered.length > visibleCount && <button type="button" className="modern-mini-btn" onClick={() => setVisibleCount(count => count + WINE_RESULTS_PAGE)}>{tx("ui-winetricks-modal", "show_more_results_of", {"math_min_visiblecount_filtered_leng": Math.min(visibleCount, filtered.length), "filtered_length": filtered.length})}</button>}
      </fieldset>
    </ModalSurface>
  </div>, document.body);
}

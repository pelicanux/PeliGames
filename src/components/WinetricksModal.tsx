import { TimedFeedback } from "./TimedFeedback";
import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { runWineTool, type WinePackage } from "../services/wineTools";
import { indexWineCatalog, searchWineCatalog, wineCatalogQuery, WINE_RESULTS_PAGE, type WineCatalogFilter } from "../services/winetricksCatalog";
import "./WinetricksModal.css";

/** Winetricks catalog of the game prefix: search, pick and install components with the
 * Proton already chosen for the game (it is chosen in the Execution options, not here). */
export function WinetricksModal({ path, name, proton, onClose }: { path: string; name: string; proton: string; onClose: () => void }) {
  const { language } = useI18n(), pt = language !== "en";
  const titleId = useId(); const terminal = useRef<HTMLPreElement>(null); const started = useRef(false);
  const [packages, setPackages] = useState<WinePackage[]>([]), [selection, setSelection] = useState<string[]>([]), [search, setSearch] = useState(""), [filter, setFilter] = useState<WineCatalogFilter>("all");
  const [visibleCount, setVisibleCount] = useState(WINE_RESULTS_PAGE);
  const [busy, setBusy] = useState(true), [error, setError] = useState(""), [lines, setLines] = useState<string[]>([]), [complete, setComplete] = useState("");
  const append = useCallback((line: string) => setLines(previous => [...previous, line].slice(-200)), []);
  useEffect(() => { if (terminal.current) terminal.current.scrollTop = terminal.current.scrollHeight; }, [lines]);
  const load = useCallback(async () => {
    const result = await runWineTool(path, "catalog", [], append); setPackages(result.packages);
  }, [path, append]);
  useEffect(() => { if (started.current) return; started.current = true; if (!proton) { setBusy(false); return; } void load().catch(e => setError(String(e))).finally(() => setBusy(false)); }, [load, proton]);
  useEffect(() => {
    const previous = document.activeElement;
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopImmediatePropagation(); if (!busy) onClose(); } };
    window.addEventListener("keydown", key, true); return () => { window.removeEventListener("keydown", key, true); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [busy, onClose]);
  const step = async (action: () => Promise<unknown>) => {
    setBusy(true); setError(""); setComplete("");
    try { await action(); } catch (failure) { setError(String(failure)); } finally { setBusy(false); }
  };
  const install = () => void step(async () => {
    const result = await runWineTool(path, "install", selection, append);
    setComplete(pt ? `Instalação concluída. Log: ${result.log}` : `Installation finished. Log: ${result.log}`); setSelection([]); await load();
  });
  const openInterface = () => { if (!busy && proton) void step(async () => { await runWineTool(path, "winetricks-gui", [], append); setSelection([]); await load(); }); };
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
  return createPortal(<div className="modal-overlay" onClick={e => { if (e.target === e.currentTarget && !busy) onClose(); }}>
    <ModalSurface className="modal-content dialog-glass winetricks-modal" role="dialog" aria-modal="true" aria-labelledby={titleId} onDismiss={() => { if (!busy) onClose(); }} closeDisabled={busy}
      header={<div className="winetricks-title"><span className="winetricks-title-icon"><MenuIcon name="cube" /></span><span><h2 id={titleId}>Winetricks</h2><small>{name}</small></span></div>}>
      <div className="winetricks-toolbar">
        <p className="winetricks-intro">{pt ? "Instale DLLs, fontes e dependências no prefixo deste jogo." : "Install DLLs, fonts and dependencies in this game's prefix."}{proton && <span className="winetricks-proton" title={proton}><MenuIcon name="cube" />{protonName}</span>}</p>
        <button type="button" className="modern-mini-btn" disabled={busy || !proton} title={pt ? "Abre o Winetricks no prefixo deste jogo. Feche a janela dele para continuar aqui." : "Opens Winetricks in this game's prefix. Close its window to continue here."} onClick={openInterface}><MenuIcon name="externalLink" />{pt ? "Abrir Winetricks" : "Open Winetricks"}</button>
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
        {!filtered.length && <p className="winetricks-empty" role="status">{busy ? (pt ? "Carregando o catálogo do Winetricks…" : "Loading the Winetricks catalog…") : !proton ? (pt ? "Escolha o Proton do jogo em Opções → Execução para ver os componentes." : "Choose the game's Proton in Options → Execution to see the components.") : !query && filter !== "installed" ? (pt ? "Digite o nome de uma DLL, fonte ou dependência para buscar." : "Enter a DLL, font or dependency name to search.") : filter === "installed" && !query ? (pt ? "Nenhum componente instalado neste prefixo." : "No components installed in this prefix.") : (pt ? "Nenhum componente encontrado." : "No components found.")}</p>}
        {filtered.length > visibleCount && <button type="button" className="modern-mini-btn" onClick={() => setVisibleCount(count => count + WINE_RESULTS_PAGE)}>{pt ? `Mostrar mais resultados (${Math.min(visibleCount, filtered.length)} de ${filtered.length})` : `Show more results (${Math.min(visibleCount, filtered.length)} of ${filtered.length})`}</button>}
      </fieldset>
      <details className="winetricks-log" open={Boolean(error) || undefined}>
        <summary>{pt ? "Registro" : "Log"}</summary>
        <pre ref={terminal} className="wine-tool-terminal" aria-label={pt ? "Log do Winetricks" : "Winetricks log"} role="log">{lines.join("\n") || (pt ? "Preparando Winetricks…" : "Preparing Winetricks…")}</pre>
      </details>
      {error && <TimedFeedback role="alert" className="advanced-validation-error">{error}</TimedFeedback>}{complete && <TimedFeedback>{complete}</TimedFeedback>}
      <div className="winetricks-actions">
        <span role="status">{busy ? (pt ? "Aguarde a operação…" : "Please wait…") : pt ? `${selection.length} selecionado(s)` : `${selection.length} selected`}</span>
        {error && !packages.length && !busy && proton && <button type="button" className="modern-mini-btn" onClick={() => void step(load)}><MenuIcon name="repair" />{pt ? "Tentar novamente" : "Try again"}</button>}
        <button type="button" className="modern-mini-btn winetricks-install" disabled={busy || !proton || !selection.length} onClick={install}><MenuIcon name="download" />{pt ? "Instalar selecionados" : "Install selected"}</button>
      </div>
    </ModalSurface>
  </div>, document.body);
}

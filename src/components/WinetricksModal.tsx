import { TimedFeedback } from "./TimedFeedback";
import { useCallback, useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { runWineTool, type WinePackage } from "../services/wineTools";
export function WinetricksModal({ path, name, onClose }: { path: string; name: string; onClose: () => void }) {
  const titleId = useId(); const terminal = useRef<HTMLPreElement>(null); const started = useRef(false);
  const [packages,setPackages] = useState<WinePackage[]>([]), [selection,setSelection] = useState<string[]>([]), [search,setSearch] = useState("");
  const [busy,setBusy] = useState(true), [error,setError] = useState(""), [lines,setLines] = useState<string[]>([]), [complete,setComplete] = useState("");
  const append = useCallback((line: string) => setLines(previous => [...previous,line].slice(-200)), []);
  useEffect(() => { if (terminal.current) terminal.current.scrollTop = terminal.current.scrollHeight; }, [lines]);
  const load = useCallback(async () => {
    const result = await runWineTool(path,"catalog",[],append); setPackages(result.packages);
  },[path,append]);
  useEffect(() => { if (started.current) return; started.current = true; void load().catch(e => setError(String(e))).finally(() => setBusy(false)); },[load]);
  useEffect(() => {
    const previous = document.activeElement;
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopImmediatePropagation(); if (!busy) onClose(); } };
    window.addEventListener("keydown",key,true); return () => { window.removeEventListener("keydown",key,true); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  },[busy,onClose]);
  const install = async () => {
    setBusy(true); setError(""); setComplete("");
    try { const result = await runWineTool(path,"install",selection,append); setComplete(`Instalação concluída. Log: ${result.log}`); setSelection([]); await load(); }
    catch(e) { setError(String(e)); } finally { setBusy(false); }
  };
  const query = search.normalize("NFD").replace(/[\u0300-\u036f]/g,"").toLowerCase();
  const filtered = packages.filter(p => `${p.id} ${p.title} ${p.category}`.normalize("NFD").replace(/[\u0300-\u036f]/g,"").toLowerCase().includes(query));
  return createPortal(<div className="modal-overlay" onClick={e => { if (e.target === e.currentTarget && !busy) onClose(); }}>
    <ModalSurface className="modal-content dialog-glass winetricks-modal" role="dialog" aria-modal="true" aria-labelledby={titleId} onDismiss={() => { if (!busy) onClose(); }} closeDisabled={busy} header={<h2 id={titleId}><MenuIcon name="cube" />Winetricks</h2>}>
      <p>Instale DLLs e fontes no prefixo de <strong>{name}</strong>, usando o Proton selecionado.</p>
      <pre ref={terminal} className="wine-tool-terminal" aria-label="Log do Winetricks" role="log">{lines.join("\n") || "Preparando Winetricks…"}</pre>
      {error && <TimedFeedback role="alert" className="advanced-validation-error">{error}</TimedFeedback>}{complete && <TimedFeedback>{complete}</TimedFeedback>}
      <label className="winetricks-search"><MenuIcon name="search" /><input type="search" aria-label="Buscar componentes Winetricks" placeholder="Buscar DLLs, fontes ou dependências…" value={search} onChange={e => setSearch(e.target.value)} /></label>
      <fieldset disabled={busy} className="winetricks-packages">
        {filtered.map(p => <label key={p.id} className="winetricks-package"><input type="checkbox" checked={p.installed || selection.includes(p.id)} disabled={p.installed} onChange={e => setSelection(previous => e.target.checked ? [...previous,p.id] : previous.filter(id => id !== p.id))} /><span><strong>{p.id}</strong><small>{p.title}</small></span><small>{p.installed ? "Instalado" : p.category === "fonts" ? "Fonte" : "DLL / componente"}</small></label>)}
        {!busy && !filtered.length && <p>Nenhum componente encontrado.</p>}
      </fieldset>
      <div className="winetricks-actions"><span role="status">{busy ? "Aguarde a operação…" : `${selection.length} selecionado(s)`}</span><button type="button" className="installation-name-confirm" disabled={busy || !selection.length} onClick={() => void install()}><MenuIcon name="download" />Instalar selecionados</button>{error && !packages.length && !busy && <button type="button" onClick={() => { setBusy(true); setError(""); void load().catch(e => setError(String(e))).finally(() => setBusy(false)); }}>Tentar novamente</button>}</div>
    </ModalSurface>
  </div>,document.body);
}

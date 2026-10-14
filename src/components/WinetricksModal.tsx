import { TimedFeedback } from "./TimedFeedback";
import { useCallback, useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { ProtonSelector } from "./ProtonSelector";
import { ProtonPreferencesDialog } from "./ProtonPreferencesDialog";
import { MenuIcon } from "./MenuIcon";
import { runWineTool, type WinePackage } from "../services/wineTools";
export function WinetricksModal({ path, name, proton, onProtonChange, onClose }: { path: string; name: string; proton: string; onProtonChange: (value: string) => Promise<void>; onClose: () => void }) {
  const titleId = useId(); const terminal = useRef<HTMLPreElement>(null); const started = useRef(false);
  const [protonManagerOpen, setProtonManagerOpen] = useState(false), [protonManagerBusy, setProtonManagerBusy] = useState(false);
  const [packages,setPackages] = useState<WinePackage[]>([]), [selection,setSelection] = useState<string[]>([]), [search,setSearch] = useState("");
  const [busy,setBusy] = useState(true), [error,setError] = useState(""), [lines,setLines] = useState<string[]>([]), [complete,setComplete] = useState("");
  const append = useCallback((line: string) => setLines(previous => [...previous,line].slice(-200)), []);
  useEffect(() => { if (terminal.current) terminal.current.scrollTop = terminal.current.scrollHeight; }, [lines]);
  const load = useCallback(async () => {
    const result = await runWineTool(path,"catalog",[],append); setPackages(result.packages);
  },[path,append]);
  useEffect(() => { if (started.current) return; started.current = true; if (!proton) { setBusy(false); return; } void load().catch(e => setError(String(e))).finally(() => setBusy(false)); },[load]);
  useEffect(() => {
    const previous = document.activeElement;
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { if (protonManagerOpen) return; e.stopImmediatePropagation(); if (!busy && !protonManagerOpen) onClose(); } };
    window.addEventListener("keydown",key,true); return () => { window.removeEventListener("keydown",key,true); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  },[busy,onClose,protonManagerOpen]);
  const changeProton = async (value: string) => {
    if (busy || value === proton) return;
    setBusy(true); setError(""); setComplete("");
    try { await onProtonChange(value); setSelection([]); await load(); }
    catch (failure) { setError(String(failure)); }
    finally { setBusy(false); }
  };
  const install = async () => {
    setBusy(true); setError(""); setComplete("");
    try { const result = await runWineTool(path,"install",selection,append); setComplete(`Instalação concluída. Log: ${result.log}`); setSelection([]); await load(); }
    catch(e) { setError(String(e)); } finally { setBusy(false); }
  };
  const openInterface = async () => {
    if (busy || !proton) return;
    setBusy(true); setError(""); setComplete("");
    try {
      await runWineTool(path,"winetricks-gui",[],append);
      setSelection([]);
      await load();
    } catch (failure) { setError(String(failure)); }
    finally { setBusy(false); }
  };
  const query = search.normalize("NFD").replace(/[\u0300-\u036f]/g,"").toLowerCase();
  const filtered = packages.filter(p => `${p.id} ${p.title} ${p.category}`.normalize("NFD").replace(/[\u0300-\u036f]/g,"").toLowerCase().includes(query));
  return createPortal(<div className="modal-overlay" onClick={e => { if (e.target === e.currentTarget && !busy && !protonManagerOpen) onClose(); }}>
    <ModalSurface className="modal-content dialog-glass winetricks-modal" role="dialog" aria-modal="true" aria-labelledby={titleId} onDismiss={() => { if (!busy && !protonManagerOpen) onClose(); }} closeDisabled={busy || protonManagerOpen} header={<h2 id={titleId}><MenuIcon name="cube" />Winetricks</h2>}>
      <p>Instale DLLs e fontes no prefixo de <strong>{name}</strong>, usando o Proton selecionado.</p>
      <ProtonSelector value={proton} onChange={value => void changeProton(value)} label="Proton" disabled={busy || protonManagerBusy} onManage={() => setProtonManagerOpen(true)} />
      <div className="winetricks-interface-action">
        <button type="button" className="installation-name-confirm" disabled={busy || protonManagerBusy || !proton} onClick={() => void openInterface()}><MenuIcon name="cube" />Abrir interface do Winetricks</button>
        <small>Abre a ferramenta original no prefixo deste jogo. Feche essa janela para continuar aqui.</small>
      </div>
      <pre ref={terminal} className="wine-tool-terminal" aria-label="Log do Winetricks" role="log">{lines.join("\n") || "Preparando Winetricks…"}</pre>
      {error && <TimedFeedback role="alert" className="advanced-validation-error">{error}</TimedFeedback>}{complete && <TimedFeedback>{complete}</TimedFeedback>}
      <label className="winetricks-search"><MenuIcon name="search" /><input type="search" aria-label="Buscar componentes Winetricks" placeholder="Buscar DLLs, fontes ou dependências…" value={search} onChange={e => setSearch(e.target.value)} /></label>
      <fieldset disabled={busy} className="winetricks-packages">
        {filtered.map(p => <label key={p.id} className="winetricks-package"><input type="checkbox" checked={p.installed || selection.includes(p.id)} disabled={p.installed} onChange={e => setSelection(previous => e.target.checked ? [...previous,p.id] : previous.filter(id => id !== p.id))} /><span><strong>{p.id}</strong><small>{p.title}</small></span><small>{p.installed ? "Instalado" : p.category === "fonts" ? "Fonte" : "DLL / componente"}</small></label>)}
        {!busy && !filtered.length && <p>Nenhum componente encontrado.</p>}
      </fieldset>
      <div className="winetricks-actions"><span role="status">{busy ? "Aguarde a operação…" : `${selection.length} selecionado(s)`}</span><button type="button" className="installation-name-confirm" disabled={busy || !proton || !selection.length} onClick={() => void install()}><MenuIcon name="download" />Instalar selecionados</button>{error && !packages.length && !busy && <button type="button" onClick={() => { setBusy(true); setError(""); void load().catch(e => setError(String(e))).finally(() => setBusy(false)); }}>Tentar novamente</button>}</div>
    </ModalSurface>
    {protonManagerOpen && <ProtonPreferencesDialog onClose={() => setProtonManagerOpen(false)} onBusyChange={setProtonManagerBusy} />}
  </div>,document.body);
}

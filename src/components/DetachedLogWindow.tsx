import { ConfirmModal } from "./ConfirmModal";
import { LogText, logSuffix } from "./LogText";
export { logLineTone } from "./LogText";
import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { MenuIcon } from "./MenuIcon";
import { CloseButton } from "./CloseButton";
import { ModalSurface } from "./ModalSurface";

export function DetachedLogWindow({ kind, target, title, onClose }: { kind: "game" | "nexus"; target: string; title: string; onClose: () => void }) {
  const close = useRef(onClose); close.current = onClose;
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    const open = async () => {
      if ("__PELI_UI_PREVIEW__" in window) {
        const url = new URL(window.location.href);
        url.searchParams.set("logWindow", kind); url.searchParams.set("target", target); url.searchParams.set("name", title);
        if (!window.open(url.toString(), `logs-${kind}-${target}`, "popup,width=900,height=620")) throw new Error("Permita abrir a janela de logs no navegador.");
      } else await invoke("open_game_log_window", { kind, target, title });
      if (active) close.current();
    };
    void open().catch(error => { if (active) setError(String(error)); });
    return () => { active = false; };
  }, [kind, target, title]);
  return error ? <div className="modal-overlay"><ModalSurface className="modal-content dialog-glass" header={<h2>Logs</h2>} onDismiss={onClose}><p role="alert">{error}</p></ModalSurface></div> : null;
}

export function closeLogWindow() {
  if ("__PELI_UI_PREVIEW__" in window) window.close();
  else void getCurrentWindow().close();
}
export function CopyLogButton({ content }: { content: string }) {
  const [copied, setCopied] = useState(false);
  const feedbackTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [error, setError] = useState("");
  const copy = async () => {
    try {
      if ("__PELI_UI_PREVIEW__" in window) await navigator.clipboard.writeText(content);
      else await invoke("plugin:clipboard-manager|write_text", { text: content });
      clearTimeout(feedbackTimer.current);
      setCopied(true); setError("");
      feedbackTimer.current = setTimeout(() => setCopied(false), 1600);
    } catch { setCopied(false); setError("Não foi possível copiar o log."); }
  };
  useEffect(() => () => clearTimeout(feedbackTimer.current), []);
  return <div className="log-copy-actions">
    <button type="button" className="btn btn-secondary log-copy-button" disabled={!content} aria-label="Copiar todo o log" title="Copiar todo o log" onClick={() => void copy()}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><rect x="8" y="8" width="12" height="13" rx="2" /><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3" /></svg>
      Copiar
    </button>
    {copied && <span className="log-copy-feedback" role="status">Copiado</span>}
    {error && <span className="log-copy-error" role="alert">{error}</span>}
  </div>;
}
export function LogWindowActions({ onClose }: { onClose: () => void }) {
  const [pinned, setPinned] = useState(false);
  const [pinBusy, setPinBusy] = useState(!("__PELI_UI_PREVIEW__" in window));
  const [pinError, setPinError] = useState("");
  useEffect(() => {
    if ("__PELI_UI_PREVIEW__" in window) return;
    let active = true;
    void getCurrentWindow().isAlwaysOnTop().then(value => { if (active) setPinned(value); }).catch(() => { if (active) setPinError("Não foi possível consultar a fixação da janela."); }).finally(() => { if (active) setPinBusy(false); });
    return () => { active = false; };
  }, []);
  const togglePin = async () => {
    if (pinBusy) return;
    setPinBusy(true);
    try {
      const next = !pinned;
      if (!("__PELI_UI_PREVIEW__" in window)) await invoke("set_log_window_on_top", { onTop: next });
      setPinned(next); setPinError("");
    } catch (error) { setPinError(`Não foi possível alterar a fixação da janela. ${String(error)}`); }
    finally { setPinBusy(false); }
  };
  return <div className="log-window-actions">
    <button type="button" className="btn-titlebar window-control log-pin-button" aria-pressed={pinned} disabled={pinBusy} aria-label={pinned ? "Desafixar janela" : "Fixar sobre todas as janelas"} title={pinned ? "Desafixar janela" : "Fixar sobre todas as janelas"} onClick={() => void togglePin()}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="m16 3 5 5-4 1-3 5v3l-7-7h3l5-3 1-4Z" /><path d="m9 15-6 6" /></svg>
    </button>
    {pinError && <span className="log-copy-error" role="alert">{pinError}</span>}
    <>
      <button type="button" className="btn-titlebar window-control" disabled={"__PELI_UI_PREVIEW__" in window} aria-label="Minimizar" title="Minimizar" onClick={() => void getCurrentWindow().minimize()}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true"><path d="M5 12h14" /></svg></button>
      <button type="button" className="btn-titlebar window-control" disabled={"__PELI_UI_PREVIEW__" in window} aria-label="Maximizar ou restaurar" title="Maximizar ou restaurar" onClick={() => void getCurrentWindow().toggleMaximize()}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2" /></svg></button>
    </>
    <CloseButton onClose={onClose} />
  </div>;
}
export function LogOutput({ content }: { content: string }) {
  const output = useRef<HTMLPreElement>(null);
  const [visibleLines,setVisibleLines]=useState(1000);
  const follow=useRef(true);
  const shown=useMemo(()=>logSuffix(content,visibleLines),[content,visibleLines]);
  useEffect(()=>{if(follow.current && output.current)output.current.scrollTop=output.current.scrollHeight;},[shown]);
  useEffect(() => {
    const select = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.key.toLowerCase() !== "a" || !output.current) return;
      event.preventDefault(); event.stopPropagation();
      output.current.focus();
      const range = document.createRange(); range.selectNodeContents(output.current);
      const selection = window.getSelection(); selection?.removeAllRanges(); selection?.addRange(range);
    };
    window.addEventListener("keydown", select, true);
    return () => window.removeEventListener("keydown", select, true);
  }, []);
  return <div className="log-output-panel"><pre ref={output} className="game-log-output" onScroll={()=>{const element=output.current;if(element)follow.current=element.scrollHeight-element.scrollTop-element.clientHeight<32;}} tabIndex={0} aria-label="Conteúdo do log"><LogText content={shown}/></pre><div className="log-level-legend" aria-label="Cores dos logs"><span className="log-line-error">Erro</span><span className="log-line-warning">Alerta</span><span className="log-line-success">Sucesso</span><span>Informação</span></div><div className="log-view-actions">{shown.length<content.length && <button type="button" className="modern-mini-btn" onClick={()=>{follow.current=false;setVisibleLines(value=>value+1000);}}>Carregar mais 1.000 linhas</button>}{visibleLines>1000 && <button type="button" className="modern-mini-btn" onClick={()=>{follow.current=true;setVisibleLines(1000);}}>Mostrar últimas 1.000</button>}<CopyLogButton content={content} /></div></div>;
}
export { WindowResizeHandles as LogResizeHandles } from "./WindowResizeHandles";
export function dragLogWindow(event: React.MouseEvent) {
  if (event.button === 0 && !(event.target as HTMLElement).closest("button") && !("__PELI_UI_PREVIEW__" in window)) void getCurrentWindow().startDragging();
}

export function OpenLogFolderButton({kind, target}: {kind: "game" | "nexus" | "launcher"; target?: string}) {
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const [confirmDelete,setConfirmDelete]=useState(false);
  const [deleted,setDeleted]=useState(false);
  const remove = async () => {
    if (busy || !target || kind === "launcher") return;
    setBusy(true);setError("");setDeleted(false);
    try {
      if ("__PELI_UI_PREVIEW__" in window) {setError("Prévia: nenhuma pasta real foi excluída.");setConfirmDelete(false);return;}
      await invoke("delete_game_logs",{kind,target});
      setConfirmDelete(false);setDeleted(true);window.dispatchEvent(new Event("gameLogsDeleted"));
    } catch (failure) {setConfirmDelete(false);setError(String(failure));}
    finally {setBusy(false);}
  };
  const open = async () => {
    if (busy) return;
    setBusy(true); setError("");
    try {
      if ("__PELI_UI_PREVIEW__" in window) { setError("Prévia: nenhuma pasta real será aberta."); return; }
      await invoke("open_log_folder", {kind, target: target ?? null});
    } catch (failure) { setError(String(failure)); }
    finally { setBusy(false); }
  };
  return <><div className="log-folder-actions"><button type="button" className="modern-mini-btn log-folder-button" disabled={busy} onClick={() => void open()} title="Abrir a pasta destes logs no gerenciador de arquivos"><MenuIcon name="folder"/>{busy ? "Aguarde…" : "Abrir pasta dos logs"}</button>{kind!=="launcher" && <button type="button" className="modern-mini-btn log-delete-button" disabled={busy || !target} onClick={()=>setConfirmDelete(true)} aria-label="Excluir pasta de logs do jogo" title="Excluir a pasta de logs deste jogo"><MenuIcon name="trash"/></button>}{deleted && <span className="log-delete-success" role="status">Pasta de logs excluída.</span>}{error && <span role="alert">{error}</span>}</div>
    {confirmDelete && <ConfirmModal title="Excluir logs deste jogo?" message="A pasta de logs deste jogo será apagada por completo, incluindo os registros atuais e anteriores. O jogo, os saves, os perfis e os backups serão preservados. Novos logs serão criados na próxima execução." confirmText="Excluir logs" cancelText="Cancelar" isDanger busy={busy} onCancel={()=>{if(!busy)setConfirmDelete(false);}} onConfirm={()=>void remove()}/>}</>;
}

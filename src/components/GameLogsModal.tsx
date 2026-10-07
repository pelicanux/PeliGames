import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { ModalSurface } from "./ModalSurface";
import type { GameInfo } from "./GameGrid";
export function GameLogsModal({ game, onClose }: { game: GameInfo; onClose: () => void }) {
  const [logs, setLogs] = useState({ current: "", previous: "" });
  const [tab, setTab] = useState<"current" | "previous">("current");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    let active = true; let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        let result;
        if ("__PELI_UI_PREVIEW__" in window) {
          const response = await fetch(`/__preview/game-logs?path=${encodeURIComponent(game.path)}`);
          const data = await response.json(); if (!response.ok || data.error) throw new Error(data.error || "Falha ao consultar logs."); result = data.result;
        } else result = await invoke<{ current: string; previous: string }>("get_peligames_game_logs", { path: game.path });
        if (active) { setLogs(result); setError(""); }
      } catch (error) { if (active) setError(String(error)); }
      finally { if (active) { setLoading(false); timer = setTimeout(poll, 1200); } }
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, [game.path]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => { if (event.key === "Escape") { event.stopPropagation(); onClose(); } };
    document.addEventListener("keydown", key); return () => document.removeEventListener("keydown", key);
  }, [onClose]);
  return createPortal(<div className="modal-overlay" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <ModalSurface role="dialog" aria-modal="true" aria-labelledby="game-log-title" className="modal-content dialog-glass game-logs-dialog" header={<h2 id="game-log-title">Logs · {game.name}</h2>} onDismiss={onClose}>
      <div className="game-log-tabs"><button aria-pressed={tab === "current"} onClick={() => setTab("current")}>Atual</button><button aria-pressed={tab === "previous"} onClick={() => setTab("previous")}>Anterior</button></div>
      <p className="game-log-hint">Atualiza durante a execução. Mantém a última execução e um backup da anterior.</p>
      {error ? <p role="alert">{error}</p> : <pre className="game-log-output" tabIndex={0}>{logs[tab] || (loading ? "Carregando…" : "Nenhum log disponível para esta execução.")}</pre>}
    </ModalSurface>
  </div>, document.body);
}

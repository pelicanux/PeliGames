import "./logWindow.css";
import { GameLogsModal } from "./GameLogsModal";
import { NexusLogsModal } from "../modules/nexus/NexusLogsModal";
import { closeLogWindow } from "./DetachedLogWindow";
import type { GameInfo } from "./GameGrid";

export function LogMonitorPage() {
  const query = new URLSearchParams(window.location.search);
  const target = query.get("target") || "";
  if (!target) return <p role="alert">Jogo não informado para os logs.</p>;
  return query.get("logWindow") === "nexus"
    ? <NexusLogsModal standalone gameId={target} onClose={closeLogWindow} />
    : <GameLogsModal standalone game={{ path: target, name: query.get("name") || "Jogo" } as GameInfo} onClose={closeLogWindow} />;
}

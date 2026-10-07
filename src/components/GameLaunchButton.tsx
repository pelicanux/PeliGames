import { TimedFeedback } from "./TimedFeedback";
import { useI18n } from "../i18n/I18nContext";
import type { useGameExecution } from "../hooks/useGameExecution";
export function GameLaunchButton({ path, execution, disabled }: { path: string; execution: ReturnType<typeof useGameExecution>; disabled?: boolean }) {
  const { t } = useI18n();
  const own = execution.session?.path === path ? execution.session : null;
  const starting = execution.pendingPath === path || own?.state === "starting";
  const active = own && ["starting", "running", "stopping"].includes(own.state);
  const stopping = own?.state === "stopping";
  return <>
    {own?.state === "running" && <span className="game-execution-status" role="status">{t("gameExecution", "running")}</span>}
    <button type="button" className={`btn btn-play game-play-button game-monitored-play ${active ? "game-monitored-cancel" : ""}`} disabled={disabled || stopping || Boolean(execution.pendingPath) || (execution.active && !active)}
      aria-busy={starting || stopping} onClick={() => void (active ? execution.cancel() : execution.start(path))}
      aria-label={t("gameExecution", stopping ? "stopping" : starting ? "starting" : active ? "cancel" : "start")}>
      {starting || stopping ? <span className="game-launch-spinner" aria-hidden="true" /> : active ? <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" /></svg> : <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z" /></svg>}
      {t("gameExecution", stopping ? "stopping" : starting ? "starting" : active ? "cancel" : "start")}
    </button>
    {(execution.error || own?.error) && <TimedFeedback>{execution.error || own?.error}</TimedFeedback>}
  </>;
}

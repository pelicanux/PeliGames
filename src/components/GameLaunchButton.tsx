import { TimedFeedback } from "./TimedFeedback";
import { useI18n } from "../i18n/I18nContext";
import type { useGameExecution } from "../hooks/useGameExecution";
export function GameLaunchButton({ path, execution, disabled }: { path: string; execution: ReturnType<typeof useGameExecution>; disabled?: boolean }) {
  const { t } = useI18n();
  const own = execution.session?.path === path ? execution.session : null;
  const starting = execution.pendingPath === path || own?.state === "starting";
  const ownActive = Boolean(own && ["starting", "running", "stopping"].includes(own.state));
  const active = starting || ownActive;
  const stopping = own?.state === "stopping" || execution.cancellingPath === path;
  return <>
    <button type="button" className={`btn btn-play game-play-button game-monitored-play ${active ? "game-monitored-cancel" : ""}${starting && !stopping ? " game-launch-preparing" : ""}`} disabled={disabled || stopping || (Boolean(execution.pendingPath) && !starting) || (execution.active && !ownActive)}
      aria-busy={starting || stopping} onClick={() => void (active ? execution.cancel() : execution.start(path))}
      aria-label={t("gameExecution", active ? "cancel" : "start")}>
      {active ? <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="5" y="5" width="14" height="14" rx="1" /></svg> : <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M5 3v18l15-9z" /></svg>}
      {t("gameExecution", active ? "cancel" : "start")}
    </button>
    {(execution.error || own?.error) && <TimedFeedback>{execution.error || own?.error}</TimedFeedback>}
  </>;
}

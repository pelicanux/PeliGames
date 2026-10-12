import { useNexusText } from "./text";
import type { DeployProgress } from "./useNexusWorkspace";

/** Stable inline file count; no filenames or growing progress panels. */
export function NexusDeployCounter({ progress }: { progress?: DeployProgress }) {
  const text = useNexusText();
  if (!progress || progress.total <= 0) return null;
  const done = Math.max(0, Math.min(progress.done, progress.total));
  return <span className="nexus-deploy-counter" role="progressbar" aria-label={text.progressFiles} aria-valuemin={0} aria-valuemax={progress.total} aria-valuenow={done}>
    {done.toLocaleString()}/{progress.total.toLocaleString()}
  </span>;
}

/** "Copiando para o jogo: 120/4567 arquivos", with a thin bar; hidden when idle. */
export function NexusDeployProgress({ progress }: { progress?: DeployProgress }) {
  const text = useNexusText();
  if (!progress || progress.total <= 0) return null;
  const label = progress.phase === "extract" ? text.progressExtract : progress.phase === "copy" ? text.progressCopy : text.progressLink;
  const format = (value: number) => value.toLocaleString();
  return <div className="nexus-deploy-progress" role="status" aria-live="polite">
    <span><strong>{label}</strong>{progress.detail && ` · ${progress.detail}`}</span>
    <small>{format(progress.done)}/{format(progress.total)} {text.progressFiles}</small>
    <progress value={progress.done} max={progress.total} />
  </div>;
}

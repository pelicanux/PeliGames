import { useEffect, useRef, useState } from "react";
import { MenuIcon } from "./MenuIcon";
import { formatTransferBytes, type TransferProgress } from "../services/downloadProgress";
import "./componentDownloadProgress.css";

export function ComponentDownloadProgress({progress,label,onDismiss}:{progress:TransferProgress;label:string;onDismiss?:()=>void}) {
  const dismiss = useRef(onDismiss);
  useEffect(() => { dismiss.current = onDismiss; }, [onDismiss]);
  const complete = progress.phase === "complete";
  const [completion, setCompletion] = useState<"visible" | "collapsing" | "hidden">("visible");
  useEffect(() => {
    setCompletion("visible");
    if (!complete) return;
    const collapse = window.setTimeout(() => setCompletion("collapsing"), 2500);
    const hide = window.setTimeout(() => { setCompletion("hidden"); dismiss.current?.(); }, 2750);
    return () => { window.clearTimeout(collapse); window.clearTimeout(hide); };
  }, [complete]);
  const transferring = progress.phase === "downloading";
  const waiting = progress.phase === "connecting";
  const failed = progress.phase === "error";
  const knownTotal = progress.total !== null && progress.total > 0;
  const percent = knownTotal ? Math.min(100, Math.max(0, progress.downloaded / progress.total! * 100)) : undefined;
  if (complete && completion === "hidden") return null;
  return <div className={`component-download-progress${complete && completion === "collapsing" ? " is-collapsing" : ""}`} data-phase={progress.phase}>
    <div className="component-download-heading"><span role="status">{complete&&<MenuIcon name="check"/>}{label}</span>{transferring&&percent!==undefined&&<span>{percent.toFixed(0)}%</span>}</div>
    {!complete&&(progress.downloaded>0||transferring)&&<div className="component-download-details"><span>{formatTransferBytes(progress.downloaded)}{knownTotal?` / ${formatTransferBytes(progress.total!)}`:""}</span>{transferring&&<span>{progress.speed_bytes_per_sec ? `${formatTransferBytes(progress.speed_bytes_per_sec)}/s` : "— /s"}</span>}</div>}
    {!failed&&!complete&&<progress aria-label={label} max={100} value={waiting||(transferring&&!knownTotal)?undefined:transferring?percent:100}/>}
  </div>;
}

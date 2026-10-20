export type WineToolPhase = "preparing" | "downloading" | "installing" | "verifying" | "interface" | "complete" | "failed" | "cancelling" | "cancelled";
export interface WineToolProgress { phase: WineToolPhase; component?: string; file?: string; percent?: number; speed?: string }
/** Only report downloader percentages, never extrapolate installation progress. */
export function wineToolProgress(previous: WineToolProgress, raw: string): WineToolProgress {
  if (["verifying", "complete", "failed", "interface", "cancelling", "cancelled"].includes(previous.phase)) return previous;
  const line = raw.replace(/\x1b\[[0-9;]*[A-Za-z]/g, "").trim();
  const component = /^Executing (?:load_|w_do_call\s+)([a-z0-9_]+)/i.exec(line)?.[1];
  if (component) return { phase: "installing", component };
  const file = /(?:Saving to:|Salvando em:)\s*[‘'"“]?([^’'"”]+?)[’'"”]?\s*$/i.exec(line)?.[1];
  if (file || /^(?:Downloading\b|Baixando\b|--\d{4}-\d{2}-\d{2} .*https?:)/i.test(line)) {
    return { phase: "downloading", component: previous.component, file };
  }
  const percent = /(?:^|\s)((?:100|[1-9]?\d)(?:\.\d+)?)%/.exec(line);
  if (previous.phase === "downloading" && percent) {
    const downloadDetails = line.slice(percent.index + percent[0].length);
    const speed = /\b\d+(?:[.,]\d+)?\s*[KMGT]?(?:i?B\/s|B\/s|B\/sec|[KMGT]\/s)\b/i.exec(downloadDetails)?.[0]
      ?? /\b\d+(?:[.,]\d+)?[KMGT](?=\s|$)/.exec(downloadDetails)?.[0];
    return { ...previous, percent: Math.min(100, Number(percent[1])), speed };
  }
  if (/^Executing\b/i.test(line) && /(?:\.(?:exe|msi)\b|\bmsiexec\b|\bwine(?:64)?\b)/i.test(line)) {
    return { phase: "installing", component: previous.component };
  }
  return previous;
}

export interface TransferProgress {
  phase: string;
  downloaded: number;
  total: number | null;
  speed_bytes_per_sec?: number;
}

export function formatTransferBytes(bytes: number): string {
  const value = Math.max(0, Number.isFinite(bytes) ? bytes : 0);
  const units = ["B", "kB", "MB", "GB"];
  const index = Math.min(3, value ? Math.floor(Math.log10(value) / 3) : 0);
  return `${(value / 1000 ** Math.max(0, index)).toFixed(index > 0 ? 1 : 0)} ${units[Math.max(0, index)]}`;
}

/** Measure delivered bytes only during transfer; never time extraction as download. */
export function createDownloadRateTracker(now: () => number = () => performance.now()) {
  let sample: { time: number; bytes: number } | undefined;
  let speed = 0;
  return <T extends TransferProgress>(progress: T): T & { speed_bytes_per_sec: number } => {
    const time = now();
    if (progress.phase === "connecting") { sample = undefined; speed = 0; }
    else if (progress.phase === "downloading") {
      if (!sample || progress.downloaded < sample.bytes) sample = { time, bytes: progress.downloaded };
      else if (time - sample.time >= 200) {
        speed = Math.max(0, progress.downloaded - sample.bytes) * 1000 / (time - sample.time);
        sample = { time, bytes: progress.downloaded };
      }
    } else if (sample) {
      if (progress.downloaded > sample.bytes && time > sample.time) {
        speed = (progress.downloaded - sample.bytes) * 1000 / (time - sample.time);
      }
      sample = undefined;
    }
    return { ...progress, speed_bytes_per_sec: speed };
  };
}

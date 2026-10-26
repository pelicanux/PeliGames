import { invoke } from "@tauri-apps/api/core";

export type LauncherUpdateInfo = {
  current: string;
  latest: string | null;
  available: boolean;
  notes: string;
  packages: { id: number; name: string; size: number }[];
  preferred: number | null;
  appimage: boolean;
  format: string;
  /** The offered version is a test (pre-release) build. */
  experimental?: boolean;
};
/** "idle": not checked yet; GitHub is consulted only on request (indicator, dialog, settings). */
export type LauncherUpdateStatus = "idle" | "checking" | "available" | "current" | "error";

export async function checkLauncherUpdate() {
    // Keep feedback visible even when GitHub responds before the first frames.
    const feedback = new Promise<void>(resolve => setTimeout(resolve, 3000));
    try { const info = await invoke<LauncherUpdateInfo>("check_launcher_update"); rememberLauncherUpdate(info); return info; }
    finally { await feedback; }
}

// At most one automatic check a day: reopening the launcher reuses the last
// answer instead of asking GitHub again. A newly installed version asks anew.
const LAST_CHECK_KEY = "peligames.launcherUpdate.lastCheck";
const DAILY = 24 * 60 * 60 * 1000;
type LastCheck = { at: number; version: string; info: LauncherUpdateInfo };
function rememberLauncherUpdate(info: LauncherUpdateInfo) {
  try { localStorage.setItem(LAST_CHECK_KEY, JSON.stringify({ at: Date.now(), version: __APP_VERSION__, info } satisfies LastCheck)); } catch { /* Checks again next time. */ }
}
export function recentLauncherUpdate(now = Date.now()): LauncherUpdateInfo | null {
  try {
    const last = JSON.parse(localStorage.getItem(LAST_CHECK_KEY) || "null") as LastCheck | null;
    if (!last?.info || last.version !== __APP_VERSION__ || last.at > now || now - last.at > DAILY) return null;
    return last.info;
  } catch { return null; }
}
let dailyRequest: Promise<LauncherUpdateInfo> | undefined;
/** The start-up answer: the last one of the day, or one new request. */
export function dailyLauncherUpdate(): Promise<LauncherUpdateInfo> {
  const recent = recentLauncherUpdate();
  if (recent) return Promise.resolve(recent);
  // Effect remounts share one request.
  return dailyRequest ??= invoke<LauncherUpdateInfo>("check_launcher_update")
    .then(info => { rememberLauncherUpdate(info); return info; })
    .finally(() => { dailyRequest = undefined; });
}

export type LauncherDownloadProgress = {
  percent: number;
  bytesPerSecond: number;
  received: number;
  total: number;
};
export function formatDownloadSpeed(bytesPerSecond: number, language: "pt" | "en") {
  const speed = Number.isFinite(bytesPerSecond) ? Math.max(0, bytesPerSecond) : 0;
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  const unit = Math.min(3, Math.floor(Math.log10(Math.max(1, speed)) / 3));
  return `${(speed / 1000 ** unit).toLocaleString(language === "pt" ? "pt-BR" : "en-US", { maximumFractionDigits: unit ? 1 : 0 })} ${units[unit]}`;
}

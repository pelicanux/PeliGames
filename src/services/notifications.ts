/** A launcher error for the notification center (bell): `source` names where it happened. */
export interface LauncherErrorDetail { source: string; message: string }
export const LAUNCHER_ERROR_EVENT = "peligamesLauncherError";
export function notifyError(source: string, error: unknown) {
  const message = String(error ?? "").replace(/^Error:\s*/, "").trim();
  if (!message) return;
  window.dispatchEvent(new CustomEvent<LauncherErrorDetail>(LAUNCHER_ERROR_EVENT, { detail: { source, message } }));
}

/** Informational feedback uses the same toast and saved notification center. */
export const LAUNCHER_NOTICE_EVENT = "peligamesLauncherNotice";
export function notifyInfo(source: string, message: string) {
  if (!message.trim()) return;
  window.dispatchEvent(new CustomEvent<LauncherErrorDetail>(LAUNCHER_NOTICE_EVENT, { detail: { source, message: message.trim() } }));
}

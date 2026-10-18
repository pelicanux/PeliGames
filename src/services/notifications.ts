/** A launcher error for the notification center (bell): `source` names where it happened. */
export interface LauncherErrorDetail { source: string; message: string; action?: "nexus-account" | "mangohud-install" }
/** A new occurrence takes priority, while the bell keeps one entry per source/message. */
export function repeatNotification<T extends { target: string; source?: string; message?: string }>(items: T[], notice: T): T[] {
  return [notice, ...items.filter(item => item.target !== notice.target || item.source !== notice.source || item.message !== notice.message)];
}
export const LAUNCHER_ERROR_EVENT = "peligamesLauncherError";
export function notifyError(source: string, error: unknown) {
  const message = String(error ?? "").replace(/^Error:\s*/, "").trim();
  if (!message) return;
  window.dispatchEvent(new CustomEvent<LauncherErrorDetail>(LAUNCHER_ERROR_EVENT, { detail: { source, message } }));
}

/** Informational feedback uses the same toast and saved notification center. */
export const LAUNCHER_NOTICE_EVENT = "peligamesLauncherNotice";
export function notifyInfo(source: string, message: string, action?: LauncherErrorDetail["action"]) {
  if (!message.trim()) return;
  window.dispatchEvent(new CustomEvent<LauncherErrorDetail>(LAUNCHER_NOTICE_EVENT, { detail: { source, message: message.trim(), action } }));
}

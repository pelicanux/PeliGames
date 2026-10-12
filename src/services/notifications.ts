/** A launcher error for the notification center (bell): `source` names where it happened. */
export interface LauncherErrorDetail { source: string; message: string }
export const LAUNCHER_ERROR_EVENT = "peligamesLauncherError";
export function notifyError(source: string, error: unknown) {
  const message = String(error ?? "").replace(/^Error:\s*/, "").trim();
  if (!message) return;
  window.dispatchEvent(new CustomEvent<LauncherErrorDetail>(LAUNCHER_ERROR_EVENT, { detail: { source, message } }));
}

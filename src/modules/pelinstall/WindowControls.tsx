import { useI18n } from "../../i18n/I18nContext";
import type { MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function dragPelinstallWindow(event: MouseEvent<HTMLDivElement>, onError: (error: string) => void) {
  if (event.button !== 0 || (event.target as Element).closest("button, input, a, select, textarea")) return;
  if ("__PELI_UI_PREVIEW__" in window) return;
  event.preventDefault();
  void getCurrentWindow().startDragging().catch(error => onError(String(error)));
}

export function PelinstallMinimizeButton({ onError }: { onError: (error: string) => void }) {
  const { language } = useI18n();
  const label = language === "pt" ? "Minimizar" : "Minimize";
  return <button type="button" className="btn-titlebar window-control pelinstall-minimize" aria-label={label} title={label}
    onClick={() => { if (!("__PELI_UI_PREVIEW__" in window)) void getCurrentWindow().minimize().catch(error => onError(String(error))); }}>
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true"><path d="M5 12h14" /></svg>
  </button>;
}

import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";

export function DiscordPanel({ onClose }: { onClose: () => void }) {
  const { language } = useI18n();
  const pt = language === "pt";
  const frame = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement;
    frame.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); onClose(); }
      if (event.key === "Tab") {
        const controls = frame.current?.querySelectorAll<HTMLElement>("button:not(:disabled), iframe");
        if (!controls?.length) return;
        const first = controls[0], last = controls[controls.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    };
    window.addEventListener("keydown", keyboard);
    return () => { window.removeEventListener("keydown", keyboard); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [onClose]);
  const join = async () => {
    const url = "https://discord.gg/78XSB9bHst";
    try { await openUrl(url); } catch { window.open(url, "_blank", "noopener,noreferrer"); }
  };
  return createPortal(<div className="modal-overlay discord-panel-overlay" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <ModalSurface ref={frame} className="modal-content dialog-glass discord-panel" role="dialog" aria-modal="true" aria-labelledby="discord-panel-heading" onDismiss={onClose}
      header={<h2 id="discord-panel-heading"><MenuIcon name="discord" />{pt ? "Comunidade PeliGames" : "PeliGames community"}</h2>}
      footer={<div className="discord-panel-footer"><button type="button" className="btn btn-primary" onClick={() => void join()}><MenuIcon name="discord" />{pt ? "Entrar no servidor" : "Join server"}</button></div>}>
      <iframe title={pt ? "Servidor Discord do PeliGames" : "PeliGames Discord server"} src="https://discord.com/widget?id=1088170837921779812&theme=dark" width="350" height="500" sandbox="allow-popups allow-popups-to-escape-sandbox allow-same-origin allow-scripts" />
    </ModalSurface>
  </div>, document.body);
}

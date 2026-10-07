import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { useEffect } from "react";

export function GameEntryModal({ onClose, onSelect }: { onClose: () => void; onSelect: (mode: "install" | "add") => void }) {
  const { t } = useI18n();
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { event.stopImmediatePropagation(); onClose(); } };
    window.addEventListener("keydown", escape, true);
    return () => window.removeEventListener("keydown", escape, true);
  }, [onClose]);
  return <div className="modal-overlay" onClick={event => { if (event.target === event.currentTarget) onClose(); }}>
    <ModalSurface className="modal-content dialog-glass mod-manager-modal" role="dialog" aria-modal="true" aria-labelledby="game-entry-title"
      onDismiss={onClose} header={<h2 id="game-entry-title">{t("gameModes", "entryTitle")}</h2>}>
      <div className="backend-options mod-manager-options">
        {(["install", "add"] as const).map(mode => <button key={mode} type="button" className="backend-option proton-preference-option mod-manager-option"
          onClick={() => onSelect(mode)}><MenuIcon name={mode === "install" ? "download" : "folder"} /><span>{t("gameModes", mode === "install" ? "install" : "addGame")}</span></button>)}
      </div>
    </ModalSurface>
  </div>;
}

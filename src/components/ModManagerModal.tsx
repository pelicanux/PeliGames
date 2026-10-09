import { useEffect, useId, useRef } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";

// Extend the catalog when another mod integration is available.
const availableMods = [{ id: "dlssnr-amd", name: "DLSSNR-AMD" }, { id: "nexus", name: "Nexus Mods" }] as const;
export type ModIntegration = typeof availableMods[number]["id"];

export function ModManagerModal({ onClose, onSelect, disabled = false }: {
  onClose: () => void; onSelect: (mod: ModIntegration) => void; disabled?: boolean;
}) {
  const { t } = useI18n();
  const titleId = useId();
  const firstOption = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const previousFocus = document.activeElement;
    firstOption.current?.focus();
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.stopImmediatePropagation(); onClose(); }
    };
    window.addEventListener("keydown", escape, true);
    return () => {
      window.removeEventListener("keydown", escape, true);
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
    };
  }, [onClose]);
  return createPortal(<div className="modal-overlay" onClick={event => {
    if (event.target === event.currentTarget) onClose();
  }}>
    <ModalSurface className="modal-content dialog-glass mod-manager-modal" role="dialog" aria-modal="true" aria-labelledby={titleId}
      onDismiss={onClose} header={<h2 id={titleId}><MenuIcon name="puzzle" />{t("gameModes", "mods")}</h2>}>
      <p className="mod-manager-prompt">{t("gameModes", "selectMod")}</p>
      <div className="backend-options mod-manager-options">
        {availableMods.map((mod, index) => <button key={mod.id} type="button" ref={index === 0 ? firstOption : undefined}
          className="backend-option proton-preference-option mod-manager-option" disabled={disabled} onClick={() => onSelect(mod.id)}>
          <>{mod.id === "nexus" ? <img className="nexus-icon" src="/nexus-mods.svg" alt="" /> : <MenuIcon name="puzzle" />}</><span>{mod.name}</span>
        </button>)}
      </div>
    </ModalSurface>
  </div>, document.body);
}

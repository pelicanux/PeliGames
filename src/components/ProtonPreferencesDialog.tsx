import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { ProtonPreferences } from "./preferences/PreferencePages";
import { usePreferencesConfig } from "./preferences/usePreferencesConfig";
import "./preferences/preferences.css";

// The standalone installer uses the same Proton controls as launcher preferences.
export function ProtonPreferencesDialog({ onClose, onBusyChange }: { onClose: () => void; onBusyChange: (busy: boolean) => void }) {
  const { t } = useI18n();
  const preferences = usePreferencesConfig(() => {});
  const [downloadBusy, setDownloadBusy] = useState(false);
  const [confirmationOpen, setConfirmationOpen] = useState(false);
  const focusRef = useRef<HTMLInputElement>(null);
  const titleId = useId();
  const busy = downloadBusy || preferences.saving;
  const close = () => { if (!busy && !preferences.isPending() && !confirmationOpen) onClose(); };
  useEffect(() => { onBusyChange(busy || confirmationOpen); return () => onBusyChange(false); }, [busy, confirmationOpen, onBusyChange]);
  useEffect(() => {
    const previous = document.activeElement;
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !confirmationOpen) { event.stopImmediatePropagation(); if (!busy && !preferences.isPending()) onClose(); }
    };
    window.addEventListener("keydown", key, true);
    return () => { window.removeEventListener("keydown", key, true); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [busy, confirmationOpen, onClose]);
  return createPortal(<div className="modal-overlay proton-preferences-overlay" onClick={event => { if (event.target === event.currentTarget) close(); }}>
    <ModalSurface className="modal-content dialog-glass proton-preferences-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}
      header={<h2 id={titleId}><MenuIcon name="download" />{t("runners", "title")}</h2>} onDismiss={close} closeDisabled={busy}>
      {preferences.error && <p role="alert">{preferences.error}</p>}
      <ProtonPreferences config={preferences.config} disabled={!preferences.ready || busy} save={preferences.save} onBusy={setDownloadBusy} focusRef={focusRef} onConfirmationChange={setConfirmationOpen} />
    </ModalSurface>
  </div>, document.body);
}

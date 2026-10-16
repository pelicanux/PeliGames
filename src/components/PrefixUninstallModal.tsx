import { ModalSurface } from "./ModalSurface";
import { useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n/I18nContext";
import type { UninstallOptions } from "../services/installedLibrary";
import "../App.css";
import "./PrefixUninstallModal.css";

interface Props {
  name: string;
  prefix: string;
  busy?: boolean;
  error?: string;
  onConfirm: (options: UninstallOptions) => void;
  onCancel: () => void;
}

/** Uninstall like Heroic: the prefix and the settings/logs are separate choices;
 * the entries and their shortcuts always leave the library. */
export function PrefixUninstallModal({ name, prefix, busy = false, error, onConfirm, onCancel }: Props) {
  const { language } = useI18n(); const pt = language !== "en";
  const [removePrefix, setRemovePrefix] = useState(false), [removeSettings, setRemoveSettings] = useState(false);
  const title = pt ? "Desinstalar?" : "Uninstall?";
  const summary = removePrefix
    ? (pt ? "O prefixo será apagado permanentemente, com o jogo, os programas e os saves que estão dentro dele." : "The prefix will be permanently deleted, with the game, programs and saves inside it.")
    : (pt ? "O prefixo e o jogo continuam no disco; só saem da biblioteca." : "The prefix and the game stay on disk; they only leave the library.");
  return createPortal(
    <div className="modal-overlay confirmation-overlay" style={{ zIndex: 9999 }}>
      <ModalSurface header={<h2 style={{ display: "flex", alignItems: "center", gap: "0.5rem", color: "var(--tone-ef4444, #ef4444)" }}>
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
            <line x1="12" y1="9" x2="12" y2="13"/>
            <line x1="12" y1="17" x2="12.01" y2="17"/>
          </svg>
          {title}
        </h2>} onDismiss={onCancel} closeDisabled={busy} role="dialog" aria-modal="true" aria-label={title} className="modal-content" style={{ maxWidth: "480px", border: "1px solid rgba(239, 68, 68, 0.3)" }}>
        <div className="modal-body" style={{ marginTop: "1rem", marginBottom: "2rem", lineHeight: "1.5" }}>
          <p className="uninstall-target"><strong>{name}</strong><br/><code>{prefix}</code></p>
          <div className="uninstall-options">
            <label className="uninstall-option"><input type="checkbox" checked={removePrefix} disabled={busy} onChange={event => setRemovePrefix(event.target.checked)}/><span><strong>{pt ? "Remover prefixo" : "Remove prefix"}</strong><small>{pt ? "Apaga a pasta do Wine/Proton e tudo o que foi instalado nela, inclusive saves." : "Deletes the Wine/Proton folder and everything installed in it, saves included."}</small></span></label>
            <label className="uninstall-option"><input type="checkbox" checked={removeSettings} disabled={busy} onChange={event => setRemoveSettings(event.target.checked)}/><span><strong>{pt ? "Apagar configurações e remover logs" : "Delete settings and remove logs"}</strong><small>{pt ? "Apaga o registro do PeliGames (installation.json) e a pasta de logs." : "Deletes the PeliGames record (installation.json) and the logs folder."}</small></span></label>
          </div>
          <p className="uninstall-summary">{summary} {pt ? "Todos os jogos que usam esse prefixo saem da biblioteca, com seus atalhos. Arquivos fora do prefixo não são apagados." : "All games using this prefix leave the library, with their shortcuts. Files outside the prefix are not deleted."}</p>
          {error && <p className="preferences-error" role="alert">{error}</p>}
        </div>
        <div className="modal-actions">
          <button disabled={busy} onClick={onCancel} style={{ background: "rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.2)" }}>{pt ? "Cancelar" : "Cancel"}</button>
          <button disabled={busy} onClick={() => onConfirm({ remove_prefix: removePrefix, remove_settings: removeSettings })} style={{ background: "var(--tone-ef4444, #ef4444)", color: "white", fontWeight: "bold" }}>{busy ? (pt ? "Desinstalando…" : "Uninstalling…") : (pt ? "Desinstalar" : "Uninstall")}</button>
        </div>
      </ModalSurface>
    </div>,
    document.body
  );
}

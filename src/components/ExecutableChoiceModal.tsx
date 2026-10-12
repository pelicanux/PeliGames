import { useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { useI18n } from "../i18n/I18nContext";
import type { GameInfo } from "./GameGrid";

/** After an installation that left several executables: the player picks the game's. */
export function ExecutableChoiceModal({ entries, prefix, busy, error, onConfirm, onKeepAll, changing, currentPath }: {
  entries: GameInfo[]; prefix: string; busy: boolean; error: string; changing?: boolean; currentPath?: string;
  onConfirm: (path: string) => void; onKeepAll: () => void;
}) {
  const { t } = useI18n();
  // Entries come ranked: shortcuts and registry hints first.
  const [chosen, setChosen] = useState((changing ? entries.find(entry => entry.path === currentPath)?.path : undefined) ?? entries.find(entry => entry.recommended)?.path ?? entries.find(entry => entry.discovery_source)?.path ?? entries[0]?.path ?? "");
  const shown = (path?: string | null) => {
    const value = path ?? "";
    const inside = prefix && value.startsWith(prefix) ? value.slice(prefix.length).replace(/^\/drive_c\//, "C:/") : value;
    return inside;
  };
  return createPortal(
    <div className="modal-overlay confirmation-overlay" style={{ zIndex: 9999 }}>
      <ModalSurface header={<h2>{t("gameModes", "chooseExecutableTitle")}</h2>} onDismiss={onKeepAll} closeDisabled={busy}
        className="modal-content executable-choice-modal">
        <p className="executable-choice-hint">{t("gameModes", changing ? "changeExecutableHint" : "chooseExecutableHint")}</p>
        <div className="executable-choice-list" role="radiogroup" aria-label={t("gameModes", "chooseExecutableTitle")}>
          {entries.map(entry => <label key={entry.path} className="nexus-fomod-option">
            <input type="radio" name="installed-executable" checked={chosen === entry.path} disabled={busy} onChange={() => setChosen(entry.path)} />
            <span>
              <strong>{(entry.executable ?? entry.path).split("/").pop()}</strong>
              {(entry.recommended || entry.discovery_source) && <small> · {t("gameModes", "chooseExecutableRecommended")}{entry.discovery_source ? ` (${entry.discovery_source})` : ""}</small>}
              <small style={{ display: "block", overflowWrap: "anywhere", opacity: 0.75 }}>{shown(entry.executable)}</small>
            </span>
          </label>)}
        </div>
        {error && <p role="alert" className="preferences-error">{error}</p>}
        <div className="executable-choice-actions">
          <button disabled={busy} onClick={onKeepAll}>{changing ? t("confirm", "cancel") : t("gameModes", "chooseExecutableKeepAll")}</button>
          <button className="primary" disabled={busy || !chosen} onClick={() => onConfirm(chosen)}>{t("gameModes", "chooseExecutableConfirm")}</button>
        </div>
      </ModalSurface>
    </div>,
    document.body,
  );
}

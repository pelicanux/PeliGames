import { useId, useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import type { GameInfo } from "./GameGrid";
import "./ExecutableChoiceModal.css";

/** Select the executable used by this installation's library shortcut. */
export function ExecutableChoiceModal({ entries, prefix, busy, error, onConfirm, onKeepAll, changing, currentPath }: {
  entries: GameInfo[]; prefix: string; busy: boolean; error: string; changing?: boolean; currentPath?: string;
  onConfirm: (path: string) => void; onKeepAll: () => void;
}) {
  const { t } = useI18n();
  const titleId = useId();
  const hintId = useId();
  const radioName = useId();
  const recommended = entries.filter(entry => entry.recommended || entry.discovery_source);
  const others = entries.filter(entry => !entry.recommended && !entry.discovery_source);
  const [chosen, setChosen] = useState((changing ? entries.find(entry => entry.path === currentPath)?.path : undefined) ?? recommended[0]?.path ?? entries[0]?.path ?? "");
  const shown = (path?: string | null) => {
    const value = path ?? "";
    const root = prefix.replace(/\/+$/, "");
    return root && value.startsWith(`${root}/`) ? value.slice(root.length).replace(/^\/drive_c\//, "C:/") : value;
  };
  const option = (entry: GameInfo, highlighted: boolean) => {
    const path = shown(entry.executable ?? entry.path);
    return <label key={entry.path} className="executable-picker-option" data-selected={chosen === entry.path} data-recommended={highlighted}>
      <input type="radio" name={radioName} checked={chosen === entry.path} disabled={busy} onChange={() => setChosen(entry.path)} />
      <span className="executable-picker-file"><MenuIcon name="play" /></span>
      <span className="executable-picker-details">
        <span className="executable-picker-name"><strong>{(entry.executable ?? entry.path).split(/[\\/]/).pop()}</strong>
          {highlighted && <span className="executable-picker-badge"><MenuIcon name="check" />{t("gameModes", "chooseExecutableRecommended")}</span>}
        </span>
        {entry.discovery_source && <span className="executable-picker-source">{entry.discovery_source}</span>}
        <small title={path}>{path}</small>
      </span>
    </label>;
  };
  return createPortal(
    <div className="modal-overlay confirmation-overlay" style={{ zIndex: 9999 }}>
      <ModalSurface header={<h2 id={titleId}><MenuIcon name="play" />{t("gameModes", "chooseExecutableTitle")}</h2>} onDismiss={onKeepAll} closeDisabled={busy}
        role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={hintId} aria-busy={busy}
        className="modal-content dialog-glass executable-picker"
        footer={<div className="executable-picker-actions">
          <button type="button" disabled={busy} onClick={onKeepAll}>{changing ? t("confirm", "cancel") : t("gameModes", "chooseExecutableKeepAll")}</button>
          <button type="button" className="executable-picker-confirm" disabled={busy || !chosen} onClick={() => onConfirm(chosen)}><MenuIcon name="check" />{t("gameModes", "chooseExecutableConfirm")}</button>
        </div>}>
        <p id={hintId} className="executable-picker-hint">{t("gameModes", changing ? "changeExecutableHint" : "chooseExecutableHint")}</p>
        <div className="executable-picker-options" role="radiogroup" aria-labelledby={titleId}>
          {recommended.map(entry => option(entry, true))}
          {recommended.length > 0 && others.length > 0 ? <details className="executable-picker-others" open={others.some(entry => entry.path === chosen) || undefined}>
            <summary>{t("gameModes", "chooseExecutableOthers")}<span>{others.length}</span></summary>
            <div className="executable-picker-options">{others.map(entry => option(entry, false))}</div>
          </details> : others.map(entry => option(entry, false))}
        </div>
        {error && <p role="alert" className="preferences-error">{error}</p>}
      </ModalSurface>
    </div>,
    document.body,
  );
}

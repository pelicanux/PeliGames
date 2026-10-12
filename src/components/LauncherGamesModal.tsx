import { useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "./ModalSurface";
import { useI18n } from "../i18n/I18nContext";
import type { LauncherGame } from "../services/installedLibrary";

/** Games a launcher (EA app, Battle.net…) installed in its prefix: the player picks which join the library. */
export function LauncherGamesModal({ games, busy, error, onAdd, onLater }: {
  games: LauncherGame[]; busy: boolean; error: string;
  onAdd: (added: LauncherGame[], declined: string[]) => void; onLater: () => void;
}) {
  const { t } = useI18n();
  const [chosen, setChosen] = useState(() => new Set(games.map(game => game.executable)));
  return createPortal(
    <div className="modal-overlay confirmation-overlay" style={{ zIndex: 9999 }}>
      <ModalSurface header={<h2>{t("gameModes", "launcherGamesTitle")}</h2>} onDismiss={onLater} closeDisabled={busy}
        className="modal-content executable-choice-modal">
        <p className="executable-choice-hint">{t("gameModes", "launcherGamesHint")}</p>
        <div className="executable-choice-list">
          {games.map(game => <label key={game.executable} className="nexus-fomod-option">
            <input type="checkbox" checked={chosen.has(game.executable)} disabled={busy}
              onChange={event => setChosen(current => { const next = new Set(current); if (event.target.checked) next.add(game.executable); else next.delete(game.executable); return next; })} />
            <span><strong>{game.name}</strong><small style={{ display: "block", overflowWrap: "anywhere", opacity: 0.75 }}>{game.executable.replace(/^.*\/drive_c\//, "C:/")}</small></span>
          </label>)}
        </div>
        {error && <p role="alert" className="preferences-error">{error}</p>}
        <div className="executable-choice-actions">
          <button disabled={busy} onClick={onLater}>{t("gameModes", "launcherGamesLater")}</button>
          <button className="primary" disabled={busy} onClick={() => onAdd(games.filter(game => chosen.has(game.executable)), games.filter(game => !chosen.has(game.executable)).map(game => game.executable))}>{t("gameModes", "launcherGamesAdd")}</button>
        </div>
      </ModalSurface>
    </div>,
    document.body,
  );
}

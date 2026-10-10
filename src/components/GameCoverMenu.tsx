import { forwardRef } from "react";
import type { GameInfo } from "./GameGrid";
import { CoverContextMenuPanel, SteamGridCoverHint } from "./CoverContextMenu";
import { CoverGameActions, type CoverGameAction } from "./CoverGameActions";
import { MenuIcon } from "./MenuIcon";
import { changeLocalCover, resetGameCover, reportCoverError } from "../services/customCovers";
import { canRemoveFromLibrary } from "../services/libraryVisibility";
import { useI18n } from "../i18n/I18nContext";
export const GameCoverMenu = forwardRef<HTMLDivElement, { game: GameInfo; x: number; y: number; blocked?: boolean; onClose: () => void; onAction: (action: CoverGameAction, game: GameInfo) => void }>(function GameCoverMenu({ game, x, y, blocked, onClose, onAction }, ref) {
  const { t, language } = useI18n();
  const action = (value: CoverGameAction) => { onClose(); onAction(value, game); };
  return <CoverContextMenuPanel ref={ref} x={x} y={y} onClose={onClose}>
    <CoverGameActions game={game} blocked={blocked} onAction={value => action(value)} />
    <SteamGridCoverHint />
    <button onClick={() => { onClose(); void changeLocalCover(game).catch(reportCoverError); }}><MenuIcon name="edit" />{t("gameGrid", "changeCover")}</button>
    <button onClick={() => { onClose(); void resetGameCover(game).catch(reportCoverError); }}><MenuIcon name="repair" />{t("gameGrid", "resetCover")}</button>
    <button disabled={blocked} onClick={() => action("scan")}><MenuIcon name="search" />{t("gameGrid", "rescan")}</button>
    {canRemoveFromLibrary(game) && <button className="cover-action-danger" disabled={blocked} onClick={() => action("remove")}><MenuIcon name="trash" />{language === "pt" ? "Remover da biblioteca" : "Remove from library"}</button>}
  </CoverContextMenuPanel>;
});

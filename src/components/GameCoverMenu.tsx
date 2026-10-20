import { forwardRef } from "react";
import type { GameInfo } from "./GameGrid";
import { CoverContextMenuPanel, SteamGridCoverHint } from "./CoverContextMenu";
import { CoverGameActions, type CoverGameAction } from "./CoverGameActions";
import { MenuIcon } from "./MenuIcon";
import { changeLocalCover, resetGameCover, reportCoverError } from "../services/customCovers";
import { canRemoveFromLibrary } from "../services/libraryVisibility";
import { useI18n } from "../i18n/I18nContext";
export const GameCoverMenu = forwardRef<HTMLDivElement, { game: GameInfo; x: number; y: number; blocked?: boolean; running?: boolean; preparing?: boolean; onClose: () => void; onAction: (action: CoverGameAction, game: GameInfo) => void }>(function GameCoverMenu({ game, x, y, blocked, running, preparing, onClose, onAction }, ref) {
  const { t, language } = useI18n();
  const own = game.launcher === "PeliGames";
  const external = game.library_view === "own" && Boolean(game.external_id);
  const steam = game.launcher === "Steam" && Boolean(game.app_id);
  const action = (value: CoverGameAction) => { onClose(); onAction(value, game); };
  return <CoverContextMenuPanel ref={ref} x={x} y={y} onClose={onClose}>
    <CoverGameActions game={game} blocked={blocked} running={running} preparing={preparing} onAction={value => action(value)} />
    <SteamGridCoverHint />
    <button onClick={() => { onClose(); void changeLocalCover(game).catch(reportCoverError); }}><MenuIcon name="edit" />{t("gameGrid", "changeCover")}</button>
    <button onClick={() => { onClose(); void resetGameCover(game).catch(reportCoverError); }}><MenuIcon name="repair" />{t("gameGrid", "resetCover")}</button>
    <button disabled={blocked} onClick={() => action("scan")}><MenuIcon name="search" />{t("gameGrid", "rescan")}</button>
    {canRemoveFromLibrary(game) && game.launcher !== "PeliGames" && <button className="cover-action-danger" disabled={blocked} onClick={() => action("remove")}><MenuIcon name="trash" />{language === "pt" ? "Remover da biblioteca" : "Remove from library"}</button>}
    {(game.library_view ?? (own ? "own" : "all")) === "own" && !external && <button className="cover-action-danger" disabled={blocked || (!own && !steam)} title={!own && !steam ? "Desinstale pelo launcher de origem." : undefined} onClick={() => action("uninstall")}><MenuIcon name="trash" />Desinstalar</button>}
  </CoverContextMenuPanel>;
});

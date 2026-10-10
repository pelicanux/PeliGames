import { useFavorites, toggleFavorite } from "../services/favorites";
import { gameViewKey } from "../services/gameIdentity";
import { useI18n } from "../i18n/I18nContext";
import type { GameInfo } from "./GameGrid";
import { MenuIcon } from "./MenuIcon";
export type CoverGameAction = "favorite" | "start" | "uninstall" | "details" | "logs" | "scan" | "remove";
export function CoverGameActions({ game, onAction, blocked }: { game: GameInfo; onAction: (action: CoverGameAction, game: GameInfo) => void; blocked?: boolean }) {
  const { t } = useI18n();
  const favorites = useFavorites();
  const favorite = favorites.has(gameViewKey(game));
  const own = game.launcher === "PeliGames";
  const nexus = game.library_view === "nexus";
  const steam = game.launcher === "Steam" && Boolean(game.app_id);
  return <div className="cover-game-actions">
    <button disabled={blocked || (!nexus && !own && !steam)} title={!nexus && !own && !steam ? "Adicione ao PeliGames para iniciar por aqui." : undefined} onClick={() => onAction("start", game)}><MenuIcon name="play" />{nexus ? "Iniciar com mods" : "Iniciar"}</button>
    <button className={`cover-favorite-action ${favorite ? "is-favorite" : ""}`} onClick={() => { toggleFavorite(game); onAction("favorite", game); }}><MenuIcon name="heart" />{t("gameGrid", favorite ? "unfavorite" : "favorite")}</button>
    {(game.library_view ?? (own ? "own" : "all")) === "own" && <button className="cover-action-danger" disabled={blocked || (!own && !steam)} title={!own && !steam ? "Desinstale pelo launcher de origem." : undefined} onClick={() => onAction("uninstall", game)}><MenuIcon name="trash" />Desinstalar</button>}
    <button onClick={() => onAction("details", game)}><MenuIcon name="info" />Detalhes</button>
    <button onClick={() => onAction("logs", game)}><MenuIcon name="logs" />Exibir logs</button>
  </div>;
}

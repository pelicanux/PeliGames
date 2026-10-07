import { useFavorites, toggleFavorite } from "../services/favorites";
import { gameViewKey } from "../services/gameIdentity";
import { useI18n } from "../i18n/I18nContext";
import type { GameInfo } from "./GameGrid";
import { MenuIcon } from "./MenuIcon";
export type CoverGameAction = "favorite" | "start" | "uninstall" | "details" | "logs";
export function CoverGameActions({ game, onAction, blocked }: { game: GameInfo; onAction: (action: CoverGameAction, game: GameInfo) => void; blocked?: boolean }) {
  const { t } = useI18n();
  const favorites = useFavorites();
  const favorite = favorites.has(gameViewKey(game));
  const own = game.launcher === "PeliGames";
  const steam = game.launcher === "Steam" && Boolean(game.app_id);
  return <div className="cover-game-actions">
    <button disabled={blocked || (!own && !steam)} title={!own && !steam ? "Adicione ao PeliGames para iniciar por aqui." : undefined} onClick={() => onAction("start", game)}><MenuIcon name="play" />Iniciar</button>
    <button className={`cover-favorite-action ${favorite ? "is-favorite" : ""}`} onClick={() => { toggleFavorite(game); onAction("favorite", game); }}><MenuIcon name="heart" />{t("gameGrid", favorite ? "unfavorite" : "favorite")}</button>
    <button className="cover-action-danger" disabled={blocked || (!own && !steam)} title={!own && !steam ? "Desinstale pelo launcher de origem." : undefined} onClick={() => onAction("uninstall", game)}><MenuIcon name="trash" />Desinstalar</button>
    <button onClick={() => onAction("details", game)}><MenuIcon name="info" />Detalhes</button>
    <button disabled={!own} title={!own ? "Logs de execução disponíveis para jogos registrados no PeliGames." : undefined} onClick={() => onAction("logs", game)}><MenuIcon name="logs" />Exibir logs</button>
  </div>;
}

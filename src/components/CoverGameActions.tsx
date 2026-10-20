import { useFavorites, toggleFavorite } from "../services/favorites";
import { gameViewKey } from "../services/gameIdentity";
import { useI18n } from "../i18n/I18nContext";
import type { GameInfo } from "./GameGrid";
import { MenuIcon } from "./MenuIcon";
export type CoverGameAction = "favorite" | "start" | "stop" | "uninstall" | "details" | "logs" | "scan" | "remove" | "duplicate";
export function CoverGameActions({ game, onAction, blocked, running, preparing }: { game: GameInfo; onAction: (action: CoverGameAction, game: GameInfo) => void; blocked?: boolean; running?: boolean; preparing?: boolean }) {
  const { t } = useI18n();
  const favorites = useFavorites();
  const favorite = favorites.has(gameViewKey(game));
  const own = game.launcher === "PeliGames";
  const nexus = game.library_view === "nexus";
  const external = game.library_view === "own" && Boolean(game.external_id);
  const steam = game.launcher === "Steam" && Boolean(game.app_id);
  const active = running || preparing;
  return <div className="cover-game-actions">
    <button className={preparing ? "game-launch-preparing" : undefined} aria-busy={preparing} disabled={(blocked && !active) || (!nexus && !own && !steam && !external)} title={!nexus && !own && !steam && !external ? "Adicione ao PeliGames para iniciar por aqui." : undefined} onClick={() => onAction(active ? "stop" : "start", game)}><MenuIcon name={active ? "stop" : "play"} />{t("gameExecution", active ? "cancel" : "start")}</button>
    <button className={`cover-favorite-action ${favorite ? "is-favorite" : ""}`} onClick={() => { toggleFavorite(game); onAction("favorite", game); }}><MenuIcon name="heart" />{t("gameGrid", favorite ? "unfavorite" : "favorite")}</button>
    {own && <button disabled={blocked || active} onClick={() => onAction("duplicate", game)}><MenuIcon name="copy" />Criar cópia…</button>}
    <button onClick={() => onAction("details", game)}><MenuIcon name="info" />Detalhes</button>
    <button onClick={() => onAction("logs", game)}><MenuIcon name="logs" />Exibir logs</button>
  </div>;
}

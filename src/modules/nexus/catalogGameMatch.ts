export interface CatalogGame { name: string; domain_name: string; vortex_supported?: boolean }
const normalize = (value: string) => value.replace(/[™®©]/g, "").normalize("NFKD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/[^a-z0-9]/g, "");
export function matchingCatalogGame(games: CatalogGame[], name: string) {
  const query = normalize(name);
  if (!query) return null;
  const matches = games.filter(game => game.vortex_supported === true && normalize(game.name) === query);
  return matches.length === 1 ? matches[0] : null;
}
export function filterCatalogGames(games: CatalogGame[], name: string) {
  const query = normalize(name);
  return games.filter(game => game.vortex_supported === true && (normalize(game.name).includes(query) || normalize(game.domain_name).includes(query)));
}

export function hasNexusCatalog(games: CatalogGame[], entry: { adapter?: string; nexus_domain?: string; game: { name: string } }) {
  const domain = entry.adapter || entry.nexus_domain;
  return domain
    ? games.some(game => game.domain_name === domain && game.vortex_supported === true)
    : Boolean(matchingCatalogGame(games, entry.game.name));
}

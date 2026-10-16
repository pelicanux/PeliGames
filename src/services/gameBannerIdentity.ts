import type { GameInfo } from "../components/GameGrid";

export function bannerSearchName(game: Pick<GameInfo, "name" | "launcher">): string {
  const name = game.name.trim();
  // Some imported entries append their launcher; do not remove game editions or sequel numbers.
  for (const separator of [" — ", " – "]) {
    const suffix = `${separator}${game.launcher}`;
    if (name.endsWith(suffix)) return name.slice(0, -suffix.length).trim();
  }
  return name;
}

export function steamIdForBanner(game: Pick<GameInfo, "launcher" | "app_id" | "cover_url" | "automatic_cover_url">): string | undefined {
  if (game.launcher === "Steam" && /^[1-9]\d*$/.test(game.app_id || "")) return game.app_id;
  for (const source of [game.automatic_cover_url, game.cover_url]) {
    if (!source) continue;
    try {
      const url = new URL(source);
      if (url.protocol !== "https:" || !["steamstatic.com", "steamcdn-a.akamaihd.net"].some(host => url.hostname === host || url.hostname.endsWith(`.${host}`))) continue;
      const match = url.pathname.match(/(?:^|\/)steam\/apps\/([1-9]\d*)\//);
      if (match) return match[1];
    } catch { /* A local cover has no store identifier. */ }
  }
}

export function gameBannerKey(game: GameInfo): string {
  const id = steamIdForBanner(game);
  return id ? `steam:${id}` : `name:${bannerSearchName(game).normalize("NFKC").toLocaleLowerCase("en-US")}`;
}

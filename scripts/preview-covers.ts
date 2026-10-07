import type { Plugin } from "vite";

// Browser preview only. Mirror the native public-catalog lookup without exposing
// credentials or adding a backend to the production bundle.
const normalize = (name: string) => name.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
const catalogName = (name: string) => normalize(name) === "godofwar2018" ? "God of War" : name.replaceAll("_", " ");
const unbrand = (name: string) => name.replace(/^(disneypixar|easports|disney)/, "");
const editions = ["standardedition", "ultimateedition", "definitiveedition", "completeedition", "enhancededition", "goldedition", "deluxeedition", "gameoftheyearedition", "gotyedition"];

export function titleScore(query: string, title: string) {
  let a = normalize(catalogName(query)), b = normalize(catalogName(title));
  if (!a) return 0;
  if (a === b) return 100;
  a = unbrand(a); b = unbrand(b);
  if (a === b) return 95;
  return a.length >= 4 && editions.some(edition => b === a + edition || a === b + edition) ? 90 : 0;
}

async function json(url: string) {
  const response = await fetch(url, { signal: AbortSignal.timeout(12000), headers: { "User-Agent": "PeliGames-preview/0.0.1" } });
  if (!response.ok) throw new Error(`Catalog request failed: ${response.status}`);
  return response.json();
}

async function steam(name: string): Promise<string | null> {
  const result = await json(`https://store.steampowered.com/api/storesearch/?term=${encodeURIComponent(catalogName(name))}&l=english&cc=US`);
  const ids = (result.items ?? []).map((item: { id: number; name: string }) => ({ id: item.id, score: titleScore(name, item.name) }))
    .filter((item: { id: number; score: number }) => item.score > 0 && Number.isInteger(item.id))
    .sort((a: { score: number }, b: { score: number }) => b.score - a.score).slice(0, 3).map((item: { id: number }) => item.id);
  if (!ids.length) return null;
  const input = { ids: ids.map((appid: number) => ({ appid })), context: { language: "english", country_code: "US" }, data_request: { include_assets: true } };
  const assets = await json(`https://api.steampowered.com/IStoreBrowseService/GetItems/v1/?input_json=${encodeURIComponent(JSON.stringify(input))}`);
  for (const id of ids) {
    const item = assets.response?.store_items?.find((item: { appid: number; success: number }) => item.appid === id && item.success === 1);
    const template = item?.assets?.asset_url_format, file = item?.assets?.library_capsule;
    if (typeof template === "string" && template.startsWith(`steam/apps/${id}/`) && template.includes("${FILENAME}") && typeof file === "string" && file && !file.includes("..")) {
      return `https://shared.akamai.steamstatic.com/store_item_assets/${template.replace("${FILENAME}", file)}`;
    }
  }
  return null;
}

async function epic(name: string): Promise<string | null> {
  const query = `query { Catalog { searchStore(keywords:${JSON.stringify(catalogName(name))}, country:"US", locale:"en-US", count:10) { elements { title keyImages { type url } } } } }`;
  const data = await json(`https://store.epicgames.com/graphql?query=${encodeURIComponent(query)}`);
  const games = (data.data?.Catalog?.searchStore?.elements ?? []).map((game: { title: string; keyImages: { type: string; url: string }[] }) => {
    const url = ["DieselGameBoxTall", "OfferImageTall", "DieselStoreFrontTall"].map(type => game.keyImages?.find(image => image.type === type && image.url.startsWith("https://"))?.url).find(Boolean);
    return { score: titleScore(name, game.title), url };
  }).filter((game: { score: number; url?: string }) => game.score > 0 && game.url).sort((a: { score: number }, b: { score: number }) => b.score - a.score);
  return games[0]?.url ?? null;
}

async function gog(name: string): Promise<string | null> {
  const data = await json(`https://catalog.gog.com/v1/catalog?query=${encodeURIComponent(`like:${catalogName(name)}`)}&limit=10&productType=in%3Agame%2Cpack&countryCode=US&locale=en-US&currencyCode=USD`);
  const games = (data.products ?? []).map((game: { title: string; coverVertical?: string }) => ({ score: titleScore(name, game.title), url: game.coverVertical }))
    .filter((game: { score: number; url?: string }) => game.score > 0 && game.url?.startsWith("https://")).sort((a: { score: number }, b: { score: number }) => b.score - a.score);
  return games[0]?.url ?? null;
}

export async function findPreviewCover(name: string): Promise<string | null> {
  let successfulCatalog = false;
  try {
    const cover = await steam(name);
    successfulCatalog = true;
    if (cover) return cover;
  } catch { /* Try the other public catalogs. */ }
  const results = await Promise.allSettled([epic(name), gog(name)]);
  for (const result of results) {
    if (result.status === "fulfilled") {
      successfulCatalog = true;
      if (result.value) return result.value;
    }
  }
  if (!successfulCatalog) throw new Error("Os catálogos não responderam. Tente novamente.");
  return null;
}

export function previewCovers(): Plugin {
  return {
    name: "peligames-preview-covers",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use(async (request, response, next) => {
        const url = new URL(request.url ?? "/", "http://localhost");
        if (url.pathname !== "/__preview/cover") return next();
        response.setHeader("Content-Type", "application/json; charset=utf-8");
        response.setHeader("Cache-Control", "no-store");
        const name = url.searchParams.get("name")?.trim();
        if (request.method !== "GET" || !name || name.length > 256) {
          response.statusCode = 400;
          response.end(JSON.stringify({ error: "Informe um nome com até 256 caracteres." }));
          return;
        }
        try { response.end(JSON.stringify({ cover: await findPreviewCover(name) })); }
        catch { response.statusCode = 502; response.end(JSON.stringify({ error: "Não foi possível consultar os catálogos de capas." })); }
      });
    },
  };
}

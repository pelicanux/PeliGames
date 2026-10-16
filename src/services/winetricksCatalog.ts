import type { WinePackage } from "./wineTools";

export type WineCatalogFilter = "all" | "dlls" | "fonts" | "installed";
export const WINE_RESULTS_PAGE = 50;
export const wineCatalogQuery = (text: string) => text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").trim().toLowerCase();
export function indexWineCatalog(packages: WinePackage[]) {
  return packages.map(item => ({ ...item, searchText: wineCatalogQuery(`${item.id} ${item.title} ${item.category}`) }));
}
/** No unrequested full catalog in the DOM. Installed items are an explicit view. */
export function searchWineCatalog(packages: ReturnType<typeof indexWineCatalog>, query: string, filter: WineCatalogFilter) {
  if (!query && filter !== "installed") return [];
  return packages.filter(item => (filter === "all" || (filter === "fonts" ? item.category === "fonts" : filter === "installed" ? item.installed : item.category !== "fonts")) && item.searchText.includes(query));
}

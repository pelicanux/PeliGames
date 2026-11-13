import type {AddonCategory, AddonManifest, AddonOverview, UiContribution} from "./contracts";

export const categories: readonly AddonCategory[] = ["mods", "frame-generation", "captures"];
/** Card copy belongs to each package and follows the host's resolved language. */
export function contributionDescription(entry: UiContribution, language: string): string {
  const descriptions = entry.contribution.descriptions;
  return descriptions?.[language] || descriptions?.[language.split("-")[0]] || descriptions?.en || entry.contribution.title;
}
export const gameTabTargets: Readonly<Record<string, UiContribution["contribution"]["target"]>> = {
  nexus: "game-mods",
  "frame-generation": "game-frame-generation",
  captures: "game-captures",
};
export function manifestCategory(manifest: AddonManifest): AddonCategory {
  if (manifest.category) return manifest.category;
  if (manifest.contributions.some(item => item.target === "game-frame-generation")) return "frame-generation";
  if (manifest.contributions.some(item => item.target === "game-captures")) return "captures";
  return "mods";
}
export function gameContributions(overview: AddonOverview | undefined, target: UiContribution["contribution"]["target"]): UiContribution[] {
  return overview?.contributions.filter(item => item.contribution.target === target) ?? [];
}
/** Keep existing integrations available until their replacement is extracted. */
export function usesBundledTarget(overview: AddonOverview | undefined, target: UiContribution["contribution"]["target"]): boolean {
  return !overview || Boolean(overview.bundled_targets?.includes(target));
}
/** Registry selection, not downloaded game DLLs, controls which tabs exist. */
export function visibleGameTabs<T extends {id: string}>(tabs: T[], overview: AddonOverview | undefined): T[] {
  if (!overview) return tabs;
  return tabs.filter(tab => !gameTabTargets[tab.id] || usesBundledTarget(overview,gameTabTargets[tab.id]) || gameContributions(overview, gameTabTargets[tab.id]).length > 0);
}
/** Keep delisted installed packages manageable, without inventing artifacts. */
export function storeEntries(overview: AddonOverview) {
  const entries = new Map(overview.catalog.addons.map(entry => [entry.manifest.id, entry]));
  for (const [id, installed] of Object.entries(overview.state.addons)) {
    const manifest = installed.versions[installed.selected ?? ""]?.manifest ?? Object.values(installed.versions)[0]?.manifest;
    if (manifest && !entries.has(id)) entries.set(id, {manifest, artifacts: {}});
  }
  return [...entries.values()];
}

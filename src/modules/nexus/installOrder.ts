import { nexusTarget } from "./nexusLinks";
import type { NexusGame } from "./useNexusWorkspace";
import type { Plan } from "./NexusInstallPlan";
import type { Requirements } from "./NexusRequirements";

export async function localInstallOrder(game: NexusGame, modId: string, fetchPlan: (id: string) => Promise<Plan>, active: () => boolean, cycleMessage: string): Promise<string[]> {
  const domain = game.adapter || game.nexus_domain;
  const visit = async (id: string, visiting: Set<string>, done: Set<string>, ordered: string[]) => {
    if (!active() || done.has(id)) return;
    if (visiting.has(id)) throw new Error(cycleMessage);
    const mod = game.mods.find(item => item.id === id);
    if (!mod) return;
    if (mod.installed && mod.enabled && id !== modId) { done.add(id); return; }
    visiting.add(id);
    const plan = mod.installed ? null : await fetchPlan(id);
    if (!active()) return;
    const required: Requirements["items"] = [
      ...(plan?.missing.map(item => ({name: item.name, url: item.url, kind: item.kind, notes: ""})) || []),
      ...(mod.nexus_source?.requirements.items.filter(item => !/\boptional\b|opcional/i.test(item.notes)) || []),
    ];
    for (const item of required) {
      const target = item.kind === "nexus" ? nexusTarget(item.url ?? null) : null;
      if (!target || target.domain !== domain) continue;
      if (game.detected_mods?.some(mod => mod.mod_id === target.id && mod.enabled)) continue;
      const matches = game.mods.filter(local => local.nexus_source?.domain === target.domain && local.nexus_source.mod_id === target.id);
      const local = matches.find(local => local.installed && local.enabled) || matches.find(local => local.installed) || matches[0];
      if (local && local.id !== id) await visit(local.id, visiting, done, ordered);
    }
    visiting.delete(id); done.add(id); ordered.push(id);
  };
  const ordered: string[] = [];
  await visit(modId, new Set(), new Set(), ordered);
  return ordered;
}

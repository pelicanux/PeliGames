import { nexusTarget } from "./nexusLinks";
import type { LocalMod, NexusWorkspace } from "./useNexusWorkspace";

/** Keep unrelated priorities stable; preserve cyclic groups without claiming a valid order inside them. */
export function orderModsByRequirements(mods: LocalMod[]): LocalMod[] {
  const dependencies = mods.map(mod => (mod.nexus_source?.requirements.items || [])
    .filter(item => item.kind === "nexus")
    .flatMap(item => {
      const target = nexusTarget(item.url || null);
      return target ? mods.flatMap((candidate, index) => candidate.nexus_source?.domain.toLowerCase() === target.domain.toLowerCase()
        && candidate.nexus_source.mod_id === target.id ? [index] : []) : [];
    }));
  const visited = new Set<number>(), finished: number[] = [];
  const visit = (node: number, graph: number[][], result: number[]) => {
    if (visited.has(node)) return;
    visited.add(node);
    graph[node].forEach(next => visit(next, graph, result));
    result.push(node);
  };
  dependencies.forEach((_, node) => visit(node, dependencies, finished));
  const reverse: number[][] = mods.map(() => []);
  dependencies.forEach((deps, node) => deps.forEach(dep => reverse[dep].push(node)));
  visited.clear();
  const groups: number[][] = [], membership: number[] = [];
  finished.reverse().forEach(node => {
    if (visited.has(node)) return;
    const members: number[] = [];
    visit(node, reverse, members);
    members.sort((a, b) => a - b);
    members.forEach(member => { membership[member] = groups.length; });
    groups.push(members);
  });
  const remaining = new Set(groups.map((_, index) => index));
  const order: number[] = [];
  while (remaining.size) {
    const ready = [...remaining].filter(group => groups[group].every(node => dependencies[node].every(dep => membership[dep] === group || !remaining.has(membership[dep]))))
      .sort((a, b) => groups[a][0] - groups[b][0])[0];
    remaining.delete(ready);
    order.push(...groups[ready]);
  }
  return order.map(index => mods[index]);
}
export function orderWorkspaceMods(workspace: NexusWorkspace): NexusWorkspace {
  return { ...workspace, games: workspace.games.map(game => ({ ...game, mods: orderModsByRequirements(game.mods) })) };
}

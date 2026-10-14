import { isObsoleteNexusFile, nexusFileCategory } from "./fileGroups";
import { nexusTarget, safeLink } from "./nexusLinks";
import { supportedNexusFile } from "./archiveFormats";
import type { Requirements } from "./NexusRequirements";
import type { NexusGame } from "./useNexusWorkspace";
export interface QueueDetails { requirements_file_id?: number | null; supports_vortex?: boolean; info: { name: string }; files: { file_id: number; name: string; file_name: string; description?: string; version: string; size: number; category_name?: string; category_id?: number }[]; requirements: Requirements }
export interface Entry { id: number; name: string; notes: string; parent: string; details?: QueueDetails; checked: boolean; file: number | null; installed: boolean; error?: string }
export interface External { kind: Requirements["items"][number]["kind"]; name: string; notes: string; url: string | null; installed: boolean }
/** A main-version choice is exclusive; patches, optionals and miscellaneous
 * downloads alongside it are independent. With no main category, retain the
 * choice of a standalone archive instead of making those mods undownloadable. */
export function queueFileGroups(files: QueueDetails["files"], selected?: number | null) {
  const available = files.filter(file => supportedNexusFile(file.file_name) && (!isObsoleteNexusFile(file) || file.file_id === selected));
  const hasMain = available.some(file => nexusFileCategory(file) === 1);
  const main = available.filter(file => hasMain ? [1, 4].includes(nexusFileCategory(file)) : nexusFileCategory(file) !== 3);
  const ids = new Set(main.map(file => file.file_id));
  return { main, additional: available.filter(file => !ids.has(file.file_id) && !isObsoleteNexusFile(file)) };
}
export async function collectQueue({ game, domain, modId, details, fileId, selectedFiles, additionalFiles, active, fetchDetails, limitWarning, incompleteWarning }: {
  game: NexusGame; domain: string; modId: number; details: QueueDetails; fileId?: number; selectedFiles?: Record<number, number>; additionalFiles?: Record<number, number[]>; active: () => boolean;
  fetchDetails: (id: number, fileId?: number) => Promise<QueueDetails>; limitWarning: string; incompleteWarning: string;
}) {
  const entries: Entry[] = [], external: External[] = [], seen = new Map<number, Entry>(), warnings: string[] = [];
  const visit = async (id: number, name: string, notes: string, parent: string, known?: QueueDetails, selected?: number) => {
    if (!active()) return;
    selected = selectedFiles?.[id] ?? selected;
    const existing = seen.get(id);
    if (existing) {
      if (parent && !existing.parent.split(" · ").includes(parent)) existing.parent = [existing.parent, parent].filter(Boolean).join(" · ");
      if (!/\boptional\b|opcional/i.test(notes) && !existing.installed) existing.checked = true;
      return;
    }
    if (seen.size >= 16) { warnings.push(limitWarning); return; }
    const installed = game.mods.some(m => m.nexus_source?.domain === domain && m.nexus_source.mod_id === id)
      || Boolean(game.detected_mods?.some(m => m.mod_id === id && (game.adapter || game.nexus_domain) === domain));
    const entry: Entry = { id, name, notes, parent, installed, checked: !installed && !/\boptional\b|opcional/i.test(notes), file: selected || null };
    seen.set(id, entry); entries.push(entry);
    if (installed && id !== modId) return;
    try {
      const value = known && (!selected || known.requirements_file_id === selected) ? known : await fetchDetails(id, selected);
      if (!active()) return;
      entry.details = value; entry.name = value.info.name;
      // An optional archive alone does not satisfy the main mod download.
      if (id === modId && entry.installed) {
        const local = game.mods.filter(mod => mod.nexus_source?.domain === domain && mod.nexus_source.mod_id === id);
        if (local.length && local.every(mod => nexusFileCategory(value.files.find(file => file.file_id === mod.nexus_source?.file_id) || {}) === 3)) {
          entry.installed = false;
          entry.checked = !/\boptional\b|opcional/i.test(notes);
        }
      }
      const files = queueFileGroups(value.files, selected).main;
      if (entry.file && !files.some(f => f.file_id === entry.file)) entry.file = null;
      if (!entry.file && files.some(f => f.file_id === value.requirements_file_id)) entry.file = value.requirements_file_id!;
      if (!entry.file && files.length === 1) entry.file = files[0].file_id;
      // Requirements can differ by archive. Consult each additional selection
      // as well, rather than assuming that the main file covers its needs.
      const mainDetails = entry.file && value.requirements_file_id != null && value.requirements_file_id !== entry.file
        ? await fetchDetails(id, entry.file) : value;
      const requirements = [mainDetails.requirements];
      const extraIds = new Set(queueFileGroups(value.files, entry.file).additional.map(file => file.file_id));
      for (const extra of additionalFiles?.[id] || []) {
        if (!extraIds.has(extra)) continue;
        const extraDetails = value.requirements_file_id === extra ? value : await fetchDetails(id, extra);
        if (!active()) return;
        requirements.push(extraDetails.requirements);
      }
      if (requirements.some(requirement => !requirement.complete || requirement.error)) warnings.push(`${entry.name}: ${incompleteWarning}`);
      for (const requirement of requirements.flatMap(requirement => requirement.items)) {
        const url = safeLink(requirement.url), target = requirement.kind === "nexus" ? nexusTarget(url) : null;
        if (target?.domain === domain) await visit(target.id, requirement.name, requirement.notes, entry.name);
        else if (!external.some(l => l.url === url && l.name === requirement.name)) {
          const normalize = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
          const name = normalize(requirement.name);
          const detected = Boolean(game.detected_mods?.some(m => normalize(m.name) === name || normalize(m.name) === "ue4ss" && ["ue4ss", "reue4ss"].includes(name)));
          external.push({ kind: requirement.kind, name: requirement.name, notes: requirement.notes, url, installed: detected });
        }
      }
    } catch (e) { entry.error = String(e); }
  };
  await visit(modId, details.info.name, "", "", details, fileId);
  return { entries, external, warnings: [...new Set(warnings)] };
}

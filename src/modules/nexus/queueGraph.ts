import { isObsoleteNexusFile, nexusFileCategory } from "./fileGroups";
import { nexusTarget, safeLink } from "./nexusLinks";
import { supportedNexusFile } from "./archiveFormats";
import type { Requirements } from "./NexusRequirements";
import type { NexusGame } from "./useNexusWorkspace";
export interface QueueDetails { info: { name: string }; files: { file_id: number; name: string; file_name: string; description?: string; version: string; size: number; category_name?: string; category_id?: number }[]; requirements: Requirements }
export interface Entry { id: number; name: string; notes: string; parent: string; details?: QueueDetails; checked: boolean; file: number | null; installed: boolean; error?: string }
export interface External { kind: Requirements["items"][number]["kind"]; name: string; notes: string; url: string | null; installed: boolean }
export async function collectQueue({ game, domain, modId, details, fileId, active, fetchDetails, limitWarning, incompleteWarning }: {
  game: NexusGame; domain: string; modId: number; details: QueueDetails; fileId?: number; active: () => boolean;
  fetchDetails: (id: number) => Promise<QueueDetails>; limitWarning: string; incompleteWarning: string;
}) {
  const entries: Entry[] = [], external: External[] = [], seen = new Map<number, Entry>(), warnings: string[] = [];
  const visit = async (id: number, name: string, notes: string, parent: string, known?: QueueDetails, selected?: number) => {
    if (!active()) return;
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
      const value = known || await fetchDetails(id);
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
      const files = value.files.filter(f => supportedNexusFile(f.file_name) && nexusFileCategory(f) !== 3 && (!isObsoleteNexusFile(f) || f.file_id === selected));
      if (entry.file && !files.some(f => f.file_id === entry.file)) entry.file = null;
      if (!entry.file && files.length === 1) entry.file = files[0].file_id;
      if (!value.requirements.complete || value.requirements.error) warnings.push(`${entry.name}: ${incompleteWarning}`);
      for (const requirement of value.requirements.items) {
        const url = safeLink(requirement.url), target = requirement.kind === "nexus" ? nexusTarget(url) : null;
        if (target?.domain === domain) await visit(target.id, requirement.name, requirement.notes, entry.name);
        else if (!external.some(l => l.url === url && l.name === requirement.name)) {
          const normalize = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
          const name = normalize(requirement.name);
          const detected = Boolean(game.detected_mods?.some(m => normalize(m.name) === name || game.adapter === "palworld" && normalize(m.name) === "ue4ss" && ["ue4ss", "reue4ss"].includes(name)));
          external.push({ kind: requirement.kind, name: requirement.name, notes: requirement.notes, url, installed: detected });
        }
      }
    } catch (e) { entry.error = String(e); }
  };
  await visit(modId, details.info.name, "", "", details, fileId);
  return { entries, external, warnings: [...new Set(warnings)] };
}

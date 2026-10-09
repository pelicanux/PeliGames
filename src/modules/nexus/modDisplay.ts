import type { LocalMod } from "./useNexusWorkspace";

/** Official metadata wins; legacy filenames are only a visual fallback. */
export function modDisplay(mod: LocalMod) {
  if (mod.nexus_source?.name?.trim()) return { name: mod.nexus_source.name.trim(), version: mod.version || mod.nexus_source.version || "" };
  const legacy = mod.name.match(/^(.*?)[ -]\d+[ -](\d+(?:[.-]\d+)+)[ -](?:\d{9,13}|\d{4}-\d{2}-\d{2}T).*$/);
  const trailingVersion = !mod.nexus_source ? mod.name.match(/^(.*?)-(\d+(?:\.\d+)+)$/) : null;
  const parsed = legacy || trailingVersion;
  return { name: (parsed?.[1] || mod.name).replace(/_/g, " ").replace(/\s+/g, " ").trim(), version: mod.version || mod.nexus_source?.version || parsed?.[2]?.replace(/-/g, ".") || "" };
}

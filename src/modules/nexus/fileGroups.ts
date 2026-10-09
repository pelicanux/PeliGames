export interface CategorizedFile { category_id?: number; category_name?: string }
export function nexusFileCategory(file: CategorizedFile): number {
  if ([1, 2, 3, 4].includes(file.category_id || 0)) return file.category_id!;
  const name = (file.category_name || "").toLowerCase().replace(/[_-]/g, " ").trim();
  if (/^main(?: files?)?$/.test(name)) return 1;
  if (/^old(?: versions?| files?)?$/.test(name)) return 4;
  if (/^optional(?: files?)?$/.test(name)) return 3;
  if (/^(?:update|patch)(?:s| files?)?$/.test(name)) return 2;
  return 0;
}
export function groupNexusFiles<T extends CategorizedFile>(files: T[]) {
  return [1, 3, 2, 0, 4].map(category => ({ category, files: files.filter(file => nexusFileCategory(file) === category) })).filter(group => group.files.length > 0);
}

export function isObsoleteNexusFile(file: CategorizedFile): boolean {
  return nexusFileCategory(file) === 4 || /^(?:archived|removed)(?:[ _-]+files?)?$/i.test((file.category_name || "").trim());
}

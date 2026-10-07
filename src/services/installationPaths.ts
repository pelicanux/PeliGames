/** A title becomes one directory component, never a path outside the library. */
export function installationFolderName(title: string): string {
  const normalized = title.trim().replace(/[<>:"/\\|?*\u0000-\u001f]/g, "-").replace(/^[.\s]+|[.\s]+$/g, "");
  let name = "", bytes = 0;
  for (const character of normalized) {
    const size = new TextEncoder().encode(character).length;
    if (bytes + size > 200) break;
    name += character; bytes += size;
  }
  return name.trim() || "Jogo";
}

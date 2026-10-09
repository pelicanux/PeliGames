/** Nexus returns HTML entities alongside HTML/BBCode. Always return plain text. */
export function plainNexusText(value?: string): string {
  let text = value || "";
  let decoder: HTMLTextAreaElement | undefined;
  // Some API descriptions are escaped twice. Bound decoding to avoid loops.
  for (let pass = 0; pass < 3; pass++) {
    const decoded = text.replace(/&(?:#(?:x[0-9a-f]+|[0-9]+)|[a-z][a-z0-9]+);/gi, entity => {
      if (entity.startsWith("&#")) {
        const hex = entity[2].toLowerCase() === "x";
        const point = Number.parseInt(entity.slice(hex ? 3 : 2, -1), hex ? 16 : 10);
        return point > 0 && point <= 0x10ffff && !(point >= 0xd800 && point <= 0xdfff)
          ? String.fromCodePoint(point) : entity;
      }
      if (typeof document === "undefined") return entity;
      decoder ??= document.createElement("textarea");
      // Only an entity token is parsed, never remote markup or a whole page.
      decoder.innerHTML = entity;
      // HTML can partially decode an unknown name with a legacy prefix
      // (e.g. &notARealEntity;). Preserve such tokens instead of corrupting them.
      const decoded = decoder.value;
      return decoded.length > 1 && decoded.endsWith(";") ? entity : decoded;
    });
    if (decoded === text) break;
    text = decoded;
  }
  return text.replace(/<br\s*\/?\s*>|<\/(?:p|div|li|h[1-6])\s*>/gi, "\n")
    .replace(/<!--[^]*?-->|<\/?[a-z][^>]*>/gi, "")
    .replace(/\[\/?(?:b|i|u|s|size|color|font|url|img|list|\*|quote|spoiler|center|left|right|heading)[^\]]*\]/gi, "")
    .replace(/\u00a0/g, " ");
}
export function safeLink(value?: string | null) {
  try { const url = new URL(value || ""); return ["https:", "http:"].includes(url.protocol) && !url.username && !url.password ? url.href : null; } catch { return null; }
}
export function nexusTarget(url: string | null) {
  try { const value = new URL(url || ""); const match = value.pathname.match(/^\/([a-z0-9-]+)\/mods\/(\d+)\/?$/);
    const id = Number(match?.[2]);
    return value.protocol === "https:" && ["www.nexusmods.com", "nexusmods.com"].includes(value.hostname) && match && Number.isSafeInteger(id) && id > 0 ? { domain: match[1], id } : null;
  } catch { return null; }
}

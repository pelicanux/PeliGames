/** Backend discovery strings retained for compatibility with older installations. */
export function isKnownGameInfo(value?: string | null): boolean {
  const text = value?.trim().normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/[.…]+$/g, "").trim();
  return Boolean(text && !/^(?:[-—–]|n\/a|unknown|none|not (?:detected|identified|found)|nao (?:detectad[oa]|identificad[oa]|encontrad[oa])|desconhecid[oa]|nenhum[a]?|verificando|detectando|checking|detecting|loading|carregando)$/.test(text));
}
export function getUpscalerFamilies(upscalers?: string[]): string[] {
  return [...new Set((upscalers ?? []).map(name => {
    const family = name.trim().match(/^(DLSS|FSR|XeSS)(?=$|[\s\d._-])/i)?.[1]?.toLowerCase();
    return family === "dlss" ? "DLSS" : family === "fsr" ? "FSR" : family === "xess" ? "XeSS" : name;
  }))];
}

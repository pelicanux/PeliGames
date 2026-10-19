import mangoCatalog from "../../config/mangohudOptions.json";

export const normalizePreferenceSearch = (value: string) => value.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/[_-]/g," ");
const aliases: Record<string,string[]> = {
  fps:["fps","quadros por segundo","taxa de quadros"],
  gpu:["gpu","placa de video"], cpu:["cpu","processador"],
  ram:["ram","memoria ram"], temperatura:["temperatura","temperature","temp"],
  atalho:["atalho","shortcut","tecla"], limite:["limite","limit","limitador"], limitar:["limite","limit","limitador"], travar:["limite","limitador"],
  backup:["backup","copia","recuperacao","restaurar"],
  gravar:["gravacao","gravar","logging","registro"],
};
export function matchesPreferenceSearch(query:string,...fields:string[]):boolean {
  const text=normalizePreferenceSearch(fields.join(" "));
  return normalizePreferenceSearch(query).trim().split(/\s+/).filter(Boolean).every(word=>(aliases[word]||[word]).some(term=>text.includes(term)));
}
const mangoSections:Record<string,string>={fps:"FPS quadros",visual:"aparencia visual layout cores fonte posicao",gpu:"GPU placa de video",cpu:"CPU processador memoria RAM",logs:"benchmark registro desempenho gravacao CSV",extras:"sistema extras",launcher:"launcher"};
export const mangoSearchOptions=(query:string)=>mangoCatalog.filter(option=>matchesPreferenceSearch(query,"MangoHUD monitoramento",option.key,option.label,option.kind==="toggle"?"ativar desativar ligar desligar habilitar desabilitar":"",("values" in option?option.values:[])?.join(" ")||"",mangoSections[option.section]||option.section));

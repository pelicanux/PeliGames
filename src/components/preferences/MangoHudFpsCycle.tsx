import { useEffect, useState } from "react";
import { notifyError } from "../../services/notifications";
export function normalizeFpsCycle(value: string): string | null {
  if (!value.trim()) return "";
  const parts = value.split(",").map(part=>part.trim());
  if (parts.length<2 || parts.length>8 || parts.some(part=>!/^\d{1,4}$/.test(part)||Number(part)>1000)) return null;
  const values = parts.map(Number);
  return new Set(values).size===values.length ? values.join(",") : null;
}
export function MangoHudFpsCycle({ value, onChange }: { value:string; onChange:(value:string)=>void }) {
  const [draft,setDraft] = useState(value);
  useEffect(()=>setDraft(value),[value]);
  const commit = () => {
    const normalized = normalizeFpsCycle(draft);
    if(normalized===null) { notifyError("MangoHUD", "Informe de 2 a 8 limites distintos entre 0 e 1000, separados por vírgulas. Use 0 para sem limite."); setDraft(value); }
    else { setDraft(normalized); if(normalized!==value) onChange(normalized); }
  };
  return <input className="mm-limit-input" type="text" aria-label="Lista de limites de FPS" maxLength={48} placeholder="Ex.: 30,60,120,0" value={draft} onChange={event=>setDraft(event.target.value)} onBlur={commit} onKeyDown={event=>{if(event.key==="Enter"){event.preventDefault();commit();}if(event.key==="Escape"){event.preventDefault();event.stopPropagation();setDraft(value);}}}/>;
}

import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InstalledProton } from "../../components/ProtonSelector";
import { useInstallerText } from "./installerText";

type Family = "GE" | "CachyOS" | "Steam" | "Outros";
const FAMILIES: Family[] = ["GE", "CachyOS", "Steam", "Outros"];
const familyOf = (proton: InstalledProton): Family => {
  const text = `${proton.name} ${proton.path}`.toLowerCase();
  if (text.includes("ge-proton") || text.includes("/ge-proton/")) return "GE";
  if (text.includes("cachyos")) return "CachyOS";
  if (text.includes("steamapps/common") || /^proton[\s\d]|experimental/i.test(proton.name)) return "Steam";
  return "Outros";
};
// Newest first: "Latest" builds, then the highest version number in the name.
const newestFirst = (a: InstalledProton, b: InstalledProton) =>
  Number(/latest/i.test(b.name)) - Number(/latest/i.test(a.name)) || b.name.localeCompare(a.name, undefined, { numeric: true });

/** The preferred family from Settings, else GE, else the first one installed. */
export function recommendedProton(installed: InstalledProton[], preferred?: string | null): InstalledProton | undefined {
  const sorted = [...installed].sort(newestFirst);
  const wanted: Family | null = preferred === "cachyos-proton" ? "CachyOS" : preferred === "ge-proton" ? "GE" : null;
  return (wanted && sorted.find(proton => familyOf(proton) === wanted)) || sorted.find(proton => familyOf(proton) === "GE") || sorted[0];
}

/** Proton step of the assistant: the suggestion on top, every other version in
 *  a searchable list that shows three at a time. */
export function PelinstallProtonPicker({ value, onChange, disabled, onManage }: { value: string; onChange: (path: string) => void; disabled?: boolean; onManage: () => void }) {
  const tr = useInstallerText();
  const [installed, setInstalled] = useState<InstalledProton[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [preferred, setPreferred] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [family, setFamily] = useState<Family | "Todos">("Todos");
  const [open, setOpen] = useState(false);
  useEffect(() => {
    let active = true, request = 0;
    const scan = () => {
      const current = ++request;
      setStatus("loading");
      invoke<InstalledProton[]>("scan_installed_protons").then(found => { if (active && current === request) { setInstalled(found); setStatus("ready"); } })
        .catch(() => { if (active && current === request) setStatus("error"); });
    };
    scan();
    invoke<{ preferred_proton_family?: string | null } | null>("load_app_config").then(config => { if (active) setPreferred(config?.preferred_proton_family ?? null); }).catch(() => {});
    window.addEventListener("protonRunnersChanged", scan);
    return () => { active = false; window.removeEventListener("protonRunnersChanged", scan); };
  }, []);
  const recommended = useMemo(() => recommendedProton(installed, preferred), [installed, preferred]);
  // Nothing chosen yet: the suggestion comes selected (it can be changed).
  useEffect(() => { if (!value && recommended && !disabled) onChange(recommended.path); }, [recommended?.path]); // eslint-disable-line react-hooks/exhaustive-deps
  const others = installed.filter(proton => proton.path !== recommended?.path).sort(newestFirst);
  const chosenOther = others.find(proton => proton.path === value) ?? (value && value !== recommended?.path && !installed.some(proton => proton.path === value) ? { name: value.split(/[\\/]/).pop() || value, path: value } : undefined);
  const families = FAMILIES.filter(name => others.some(proton => familyOf(proton) === name));
  const words = query.trim().toLowerCase();
  const shown = others.filter(proton => (family === "Todos" || familyOf(proton) === family) && (!words || proton.name.toLowerCase().includes(words)));
  const option = (proton: InstalledProton, detail: string, tag?: string, compact = false) =>
    <button type="button" key={proton.path} className={`pi-option${compact ? " compact" : ""}`} role="radio" aria-checked={value === proton.path} disabled={disabled} onClick={() => onChange(proton.path)} title={proton.path}>
      <span className="pi-radio" /><span className="pi-option-text"><strong>{proton.name}</strong><small>{detail}</small></span>{tag && <span className="pi-tag">{tag}</span>}
    </button>;
  if (status === "loading" && !installed.length) return <p className="pi-hint" role="status">{tr("Procurando versões do Proton…")}</p>;
  if (!installed.length) return <div className="pi-list">
    <p className="pi-hint" role="status">{tr(status === "error" ? "Não foi possível listar as versões do Proton." : "Nenhuma versão do Proton instalada.")}</p>
    <button type="button" className="pi-link" onClick={onManage}>{tr("Baixar ou gerenciar versões do Proton…")}</button>
  </div>;
  return <>
    <div className="pi-list" role="radiogroup" aria-label={tr("Versão do Proton")}>
      {recommended && option(recommended, familyOf(recommended) === "Outros" ? recommended.path : familyOf(recommended), tr("Recomendado"))}
      {chosenOther && option(chosenOther, tr("Escolhida por você"))}
    </div>
    {others.length > 0 && <details className="pi-more" open={open || Boolean(chosenOther)} onToggle={event => setOpen(event.currentTarget.open)}>
      <summary>{tr("Outras versões instaladas")} <b>{others.length}</b><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" aria-hidden="true"><path d="m9 6 6 6-6 6" /></svg></summary>
      <div className="pi-more-body">
        <label className="pi-field small"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg>
          <input value={query} onChange={event => setQuery(event.target.value)} placeholder={tr("Buscar versão")} aria-label={tr("Buscar versão")} /></label>
        {families.length > 1 && <div className="pi-family">{(["Todos", ...families] as const).map(name => <button type="button" key={name} aria-pressed={family === name} onClick={() => setFamily(name)}>{name === "Todos" ? tr("Todos") : name}</button>)}</div>}
        <div className="pi-list pi-scroll3" role="radiogroup" aria-label={tr("Outras versões instaladas")}>
          {shown.length ? shown.map(proton => option(proton, familyOf(proton) === "Outros" ? proton.path : familyOf(proton), undefined, true)) : <p className="pi-hint">{tr("Nenhuma versão encontrada.")}</p>}
        </div>
      </div>
    </details>}
    <button type="button" className="pi-link" onClick={onManage}>{tr("Baixar ou gerenciar versões do Proton…")}</button>
  </>;
}

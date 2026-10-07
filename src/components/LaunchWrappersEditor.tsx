import { useState } from "react";
import { MenuIcon } from "./MenuIcon";
import type { LaunchWrapper } from "../services/advancedSettings";

export function LaunchWrappersEditor({ value, onChange }: { value: LaunchWrapper[]; onChange: (value: LaunchWrapper[]) => boolean }) {
  const [program, setProgram] = useState("");
  const [args, setArgs] = useState("");
  const [editing, setEditing] = useState<number | null>(null);
  return <div className="advanced-wrappers-editor">
    <h4>Comando do wrapper</h4>
    <p>Executa antes do Proton. Seu script deve repassar o comando recebido para iniciar o jogo.</p>
    {value.map((wrapper, index) => <div className="advanced-wrapper-row" key={index}>
      <button type="button" role="switch" className={`advanced-toggle ${wrapper.enabled ? "enabled" : ""}`} aria-label={`Ativar wrapper ${index + 1}`} aria-checked={wrapper.enabled} onClick={() => onChange(value.map((item, i) => i === index ? { ...item, enabled: !item.enabled } : item))}><span /></button>
      <div className="advanced-wrapper-summary"><strong>{wrapper.program}</strong><small>{wrapper.arguments}</small></div>
      <button type="button" className="installation-name-confirm" aria-label={`Editar wrapper ${index + 1}`} onClick={() => { setEditing(index); setProgram(wrapper.program); setArgs(wrapper.arguments); }}><MenuIcon name="edit" /></button>
      <button type="button" className="installation-name-confirm" aria-label={`Remover wrapper ${index + 1}`} onClick={() => { onChange(value.filter((_, i) => i !== index)); setEditing(null); setProgram(""); setArgs(""); }}>−</button>
    </div>)}
    <form className="advanced-wrapper-form" onSubmit={event => {
      event.preventDefault(); if (!program.trim()) return;
      const item = { program: program.trim(), arguments: args, enabled: editing === null ? false : value[editing].enabled };
      if (!onChange(editing === null ? [...value, item] : value.map((old, i) => i === editing ? item : old))) return;
      setProgram(""); setArgs(""); setEditing(null);
    }}>
      <input aria-label="Executável do wrapper" placeholder="Novo wrapper ou caminho do script" value={program} onChange={event => setProgram(event.target.value)} />
      <input aria-label="Argumentos do wrapper" placeholder="Argumentos do wrapper" value={args} onChange={event => setArgs(event.target.value)} />
      <button type="submit" className="installation-name-confirm" disabled={!program.trim()} aria-label={editing === null ? "Adicionar wrapper" : "Salvar wrapper"}><MenuIcon name={editing === null ? "plus" : "check"} /></button>
      {editing !== null && <button type="button" onClick={() => { setEditing(null); setProgram(""); setArgs(""); }}>Cancelar</button>}
    </form>
  </div>;
}

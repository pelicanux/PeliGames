import { RetractableInput } from "../../components/RetractableInput";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LaunchOptionsSurface } from "../../components/LaunchOptionsSurface";
import { MenuIcon } from "../../components/MenuIcon";
import { useI18n } from "../../i18n/I18nContext";

type Options = { automatic: string; effective: string; customized: boolean };
export function NexusLaunchOptions({ gameId, disabled, busy, errorMessage, onSave, inline = false }: {
  inline?: boolean; gameId: string; disabled: boolean; busy: boolean; errorMessage: string;
  onSave: (gameId: string, options: string | null) => Promise<boolean>;
}) {
  const { language } = useI18n(), pt = language === "pt";
  const [open, setOpen] = useState(false);
  const [options, setOptions] = useState<Options | null>(null);
  const [draft, setDraft] = useState(""), [error, setError] = useState(""), [saving, setSaving] = useState(false);
  const title = pt ? "Opções de inicialização" : "Launch options";
  useEffect(() => { setOpen(false); setOptions(null); setError(""); }, [gameId]);
  useEffect(() => {
    if (!open && !inline) return;
    let active = true; setError("");
    void invoke<Options>("get_nexus_launch_options", { gameId }).then(value => {
      if (active) { setOptions(value); setDraft(inline && !value.customized ? "" : value.effective); }
    }).catch(error => { if (active) setError(String(error)); });
    return () => { active = false; };
  }, [gameId, open, inline]);
  const save = async (value: string | null) => {
    if (saving || busy || disabled || !options) return;
    setSaving(true);
    try { if (await onSave(gameId, value)) { setOptions({...options, effective:value ?? options.automatic, customized:value !== null}); setDraft(value ?? options.automatic); setOpen(false); } }
    finally { setSaving(false); }
  };
  if (inline) return <section className="nexus-advanced-launch-options">
    <div><h4>{title}</h4><p className="nexus-hint">{pt ? "Argumentos e variáveis deste jogo." : "Arguments and variables for this game."}</p></div>
    <RetractableInput open={open} setOpen={setOpen} label={title} value={draft} onChange={setDraft} onSubmit={() => void save(draft || null)} disabled={disabled || busy || saving || !options} error={error || errorMessage}>
      <button type="button" disabled={saving || busy || disabled} onClick={() => void save(null)}>{pt ? "Restaurar" : "Reset"}</button>
    </RetractableInput>
    {options?.customized && options.effective && <code className="modern-saved-launch-options">{options.effective}</code>}
    {error && !open && <p role="alert" className="nexus-error">{error}</p>}
  </section>;
  return <section className="nexus-advanced-launch-options">
    <div><h4><MenuIcon name="play" />{title}</h4><p className="nexus-hint">{pt ? "Confira as opções dos mods e edite os argumentos do jogo." : "Review mod launch options and edit game arguments."}</p></div>
    <button type="button" className="btn btn-secondary" disabled={disabled || busy} onClick={() => setOpen(true)}><MenuIcon name="edit" />{pt ? "Editar" : "Edit"}</button>
    {open && <LaunchOptionsSurface inline={inline} title={title} onClose={() => { if (!saving && !busy) setOpen(false); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={!options || saving || busy || disabled} onClick={() => void save(null)}>{pt ? "Restaurar automáticas" : "Restore automatic"}</button>
      <button type="button" className="btn btn-primary" disabled={!options || saving || busy || disabled} onClick={() => void save(draft === options?.automatic ? null : draft)}>{saving ? pt ? "Salvando…" : "Saving…" : pt ? "Salvar" : "Save"}</button>
    </>}>
      {!options && !error && <p role="status">{pt ? "Carregando…" : "Loading…"}</p>}
      {options && <>
        <label className="nexus-launch-options-label">{pt ? "Automáticas detectadas pelo launcher" : "Automatic options detected by the launcher"}</label>
        <pre className="nexus-launch-options-automatic">{options.automatic || (pt ? "Nenhuma opção automática adicional." : "No additional automatic options.")}</pre>
        <p className="nexus-hint">{pt ? "Edite os argumentos e variáveis abaixo. Variáveis vêm antes dos argumentos, por exemplo: MANGOHUD=1 %command% -windowed. Prioridade de DLLs é configurada em Prioridade das DLLs do Wine. Não são executados comandos de shell." : "Edit arguments and variables below. Variables precede arguments, for example: MANGOHUD=1 %command% -windowed. Configure DLL priorities under Wine DLL priority. Shell commands are not executed."}</p>
        <label className="nexus-launch-options-label" htmlFor={`launch-options-${gameId}`}>{pt ? "Opções usadas ao iniciar" : "Options used on launch"}</label>
        <textarea id={`launch-options-${gameId}`} className="nexus-launch-options-input" value={draft} maxLength={4096} rows={inline ? 3 : 5} disabled={saving || busy || disabled} onChange={event => setDraft(event.target.value)} />
        <p className="nexus-hint">{pt ? "Opções personalizadas substituem as automáticas mostradas acima. Após mudar mods, confira as opções ou restaure o modo automático. Estas opções valem apenas para iniciar por esta biblioteca Nexus." : "Custom options replace the automatic options shown above. After changing mods, review them or restore automatic mode. These options apply only when launching from this Nexus library."}</p>
      </>}
      {(error || errorMessage) && <p role="alert" className="nexus-error">{error || errorMessage}</p>}
    </LaunchOptionsSurface>}
  </section>;
}

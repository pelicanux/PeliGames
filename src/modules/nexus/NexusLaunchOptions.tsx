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
  const {t: tx} = useI18n();

  const [open, setOpen] = useState(false);
  const [options, setOptions] = useState<Options | null>(null);
  const [draft, setDraft] = useState(""), [error, setError] = useState(""), [saving, setSaving] = useState(false);
  const title = tx("ui-nexus-launch-options", "launch_options");
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
    <div><h4>{title}</h4><p className="nexus-hint">{tx("ui-nexus-launch-options", "arguments_and_variables_for_this_game")}</p></div>
    <RetractableInput open={open} setOpen={setOpen} label={title} value={draft} onChange={setDraft} onSubmit={() => void save(draft || null)} disabled={disabled || busy || saving || !options} error={error || errorMessage}>
      <button type="button" disabled={saving || busy || disabled} onClick={() => void save(null)}>{tx("ui-nexus-launch-options", "reset")}</button>
    </RetractableInput>
    {options?.customized && options.effective && <code className="modern-saved-launch-options">{options.effective}</code>}
    {error && !open && <p role="alert" className="nexus-error">{error}</p>}
  </section>;
  return <section className="nexus-advanced-launch-options">
    <div><h4><MenuIcon name="play" />{title}</h4><p className="nexus-hint">{tx("ui-nexus-launch-options", "review_mod_launch_options_and_edit_game_arguments")}</p></div>
    <button type="button" className="btn btn-secondary" disabled={disabled || busy} onClick={() => setOpen(true)}><MenuIcon name="edit" />{tx("ui-nexus-launch-options", "edit")}</button>
    {open && <LaunchOptionsSurface inline={inline} title={title} onClose={() => { if (!saving && !busy) setOpen(false); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={!options || saving || busy || disabled} onClick={() => void save(null)}>{tx("ui-nexus-launch-options", "restore_automatic")}</button>
      <button type="button" className="btn btn-primary" disabled={!options || saving || busy || disabled} onClick={() => void save(draft === options?.automatic ? null : draft)}>{saving ? tx("ui-nexus-launch-options", "saving") : tx("ui-nexus-launch-options", "save")}</button>
    </>}>
      {!options && !error && <p role="status">{tx("ui-nexus-launch-options", "loading")}</p>}
      {options && <>
        <label className="nexus-launch-options-label">{tx("ui-nexus-launch-options", "automatic_options_detected_by_the_launcher")}</label>
        <pre className="nexus-launch-options-automatic">{options.automatic || (tx("ui-nexus-launch-options", "no_additional_automatic_options"))}</pre>
        <p className="nexus-hint">{tx("ui-nexus-launch-options", "edit_arguments_and_variables_below_variables_precede_arguments_for_example_mangohud_1_co")}</p>
        <label className="nexus-launch-options-label" htmlFor={`launch-options-${gameId}`}>{tx("ui-nexus-launch-options", "options_used_on_launch")}</label>
        <textarea id={`launch-options-${gameId}`} className="nexus-launch-options-input" value={draft} maxLength={4096} rows={inline ? 3 : 5} disabled={saving || busy || disabled} onChange={event => setDraft(event.target.value)} />
        <p className="nexus-hint">{tx("ui-nexus-launch-options", "custom_options_replace_the_automatic_options_shown_above_after_changing_mods_review_them")}</p>
      </>}
      {(error || errorMessage) && <p role="alert" className="nexus-error">{error || errorMessage}</p>}
    </LaunchOptionsSurface>}
  </section>;
}

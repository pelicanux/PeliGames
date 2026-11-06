import { invoke } from "@tauri-apps/api/core";
import { notifyError } from "../services/notifications";
import { RetractableInput } from "./RetractableInput";
import { useState } from "react";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";
import { LaunchOptionsSurface } from "./LaunchOptionsSurface";
import type { GameInfo } from "./GameGrid";

export function InstalledLaunchOptions({ game, disabled, onSave, errorMessage, inline = false }: {
  inline?: boolean; game: GameInfo; disabled: boolean; errorMessage: string; onSave: (options: string | null) => Promise<boolean>;
}) {
  const {t: tx} = useI18n();

  const [open, setOpen] = useState(false), [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const title = tx("ui-installed-launch-options", "launch_options");
  const automatic = (game.launch_arguments ?? []).map(arg => JSON.stringify(arg)).join(" ");
  // Back to zero, keeping only what installed mods (DLSSNR-AMD) need to load.
  const restore = async () => {
    if (saving || disabled) return;
    let kept: string | null;
    try { kept = "__PELI_UI_PREVIEW__" in window ? null : await invoke<string | null>("mod_launch_options_for", { gameDir: game.directory || game.path.replace(/\/[^/]*$/, "") }); }
    catch (error) { notifyError(title, error); return; }
    await save(kept);
  };
  const restoreHint = tx("ui-installed-launch-options", "clears_what_was_typed_lines_of_installed_mods_such_as_dlssnr_amd_are_kept");
  const save = async (value: string | null) => {
    if (saving || disabled) return;
    setSaving(true);
    try { if (await onSave(value)) setOpen(false); }
    finally { setSaving(false); }
  };
  if (inline) return <section className="nexus-advanced-launch-options">
    <div><h4>{title}</h4><p className="nexus-hint">{tx("ui-installed-launch-options", "arguments_and_variables_for_this_game")}</p></div>
    <RetractableInput open={open} setOpen={setOpen} label={title} value={draft} onChange={setDraft} onOpen={() => setDraft(game.advanced?.launch_options ?? "")} onSubmit={() => void save(draft || null)} disabled={disabled || saving} error={errorMessage}>
      <button type="button" disabled={saving || disabled} title={restoreHint} onClick={() => void restore()}>{tx("ui-installed-launch-options", "reset")}</button>
    </RetractableInput>
    {game.advanced?.launch_options && <code className="modern-saved-launch-options">{game.advanced.launch_options}</code>}
  </section>;
  return <section className="nexus-advanced-launch-options">
    <div><h4><MenuIcon name="play" />{title}</h4><p className="nexus-hint">{tx("ui-installed-launch-options", "review_and_edit_game_arguments_and_variables")}</p></div>
    <button type="button" className="btn btn-secondary" disabled={disabled} onClick={() => { setDraft(game.advanced?.launch_options ?? (inline ? "" : automatic)); setOpen(true); }}><MenuIcon name="edit" />{tx("ui-installed-launch-options", "edit")}</button>
    {open && <LaunchOptionsSurface inline={inline} title={title} onClose={() => { if (!saving) setOpen(false); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={saving || disabled} title={restoreHint} onClick={() => void restore()}>{tx("ui-installed-launch-options", "restore_automatic")}</button>
      <button type="button" className="btn btn-primary" disabled={saving || disabled} onClick={() => void save(draft === automatic ? null : draft)}>{saving ? tx("ui-installed-launch-options", "saving") : tx("ui-installed-launch-options", "save")}</button>
    </>}>
      <label className="nexus-launch-options-label">{tx("ui-installed-launch-options", "arguments_detected_in_the_game_shortcut")}</label>
      <pre className="nexus-launch-options-automatic">{automatic || (tx("ui-installed-launch-options", "no_additional_automatic_arguments"))}</pre>
      <p className="nexus-hint">{tx("ui-installed-launch-options", "variables_precede_arguments_for_example_mangohud_1_command_windowed_configure_dll_priori")}</p>
      <label className="nexus-launch-options-label" htmlFor="installed-launch-options">{tx("ui-installed-launch-options", "options_used_on_launch")}</label>
      <textarea id="installed-launch-options" className="nexus-launch-options-input" rows={inline ? 3 : 5} maxLength={4096} value={draft} disabled={saving || disabled} onChange={event => setDraft(event.target.value)} />
      <p className="nexus-hint">{tx("ui-installed-launch-options", "custom_options_replace_automatic_arguments_when_launching_this_game_from_the_main_librar")}</p>
      {errorMessage && <p role="alert" className="nexus-error">{errorMessage}</p>}
    </LaunchOptionsSurface>}
  </section>;
}

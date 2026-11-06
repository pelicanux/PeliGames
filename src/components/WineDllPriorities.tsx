import { RetractableInput } from "./RetractableInput";
import { useEffect, useRef, useState } from "react";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import type { AdvancedSettings } from "../services/advancedSettings";

/** Separate editing visibility from the saved priorities applied at launch. */
export function WineDllPriorities({ settings, disabled, automatic, detectionError, onChange, onReset, compact = false }: {
  compact?: boolean; settings: AdvancedSettings; disabled?: boolean; automatic?: string; detectionError?: string;
  onReset?: () => void;
  onChange: (settings: AdvancedSettings) => boolean;
}) {
  const {t: tx} = useI18n();

  const [editing, setEditing] = useState(false), [draft, setDraft] = useState("");
  const [error, setError] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { if(editing && !disabled) input.current?.focus(); }, [editing, disabled]);
  const entries = (value: string) => value.split(";").map(entry => entry.trim()).filter(Boolean);
  const manual = settings.options.dll_overrides ? entries(settings.dll_overrides) : [];
  const detected = entries(automatic ?? "");
  const effective = new Map<string, string>();
  for (const entry of [...detected, ...manual]) {
    const [names, order] = entry.split("=");
    for (const name of names.split(",")) effective.set(name.toLowerCase(), order);
  }
  const displayed = [...effective].map(([name, order]) => `${name}=${order}`);
  const title = tx("ui-wine-dll-priorities", "wine_dll_priority");
  const save = (next: string[]) => onChange({ ...settings, dll_overrides: next.join(";"), options: { ...settings.options, dll_overrides: next.length > 0 } });
  const add = () => {
    const value = draft.trim();
    if (!value || disabled) return;
    if (!/^[\w.*-]+(?:,[\w.*-]+)*=(?:n|b|d|n,b|b,n)?(?:;[\w.*-]+(?:,[\w.*-]+)*=(?:n|b|d|n,b|b,n)?)*$/.test(value)) {
      setError(tx("ui-wine-dll-priorities", "use_the_format_dxgi_n_b_dinput8_n_b")); return;
    }
    // Each name has one priority; adding it again updates its existing pill.
    const priorities = new Map<string, string>();
    for (const entry of [...manual, ...entries(value)]) {
      const [names, order] = entry.split("=");
      for (const name of names.split(",")) priorities.set(name.toLowerCase(), order);
    }
    if (save([...priorities].map(([name, order]) => `${name}=${order}`))) { setDraft(""); setError(""); if (compact) setEditing(false); }
  };
  if (compact) return <div className="advanced-option advanced-wine-dlls modern-inline-dlls">
    <div className="advanced-option-description"><label>{title}</label></div>
    <RetractableInput open={editing} setOpen={setEditing} label={title} value={draft} onChange={value => { setDraft(value); setError(""); }} onOpen={() => { setDraft(""); setError(""); }} onSubmit={add} disabled={disabled} error={error || detectionError} maxLength={2048} />
    {displayed.length > 0 && <div className="advanced-dll-summary"><div className="advanced-dll-pills">
      {displayed.map(entry => <span className={`advanced-dll-pill${manual.includes(entry) ? "" : " detected"}`} key={entry}>{entry}{manual.includes(entry)
        ? <button type="button" disabled={disabled} aria-label={`${tx("ui-wine-dll-priorities", "remove")} ${entry}`} onClick={() => save(manual.filter(item => item !== entry))}><span aria-hidden="true">×</span></button>
        : <small>mod</small>}</span>)}
    </div></div>}
    {manual.length > 0 && <div className="modern-dll-reset"><button type="button" className="modern-mini-btn" disabled={disabled} onClick={() => { if (save([])) onReset?.(); }}>{tx("ui-wine-dll-priorities", "reset")}</button></div>}
    {detectionError && !editing && <p role="alert" className="advanced-dll-error">{detectionError}</p>}
  </div>;
  return <div className={`advanced-option advanced-wine-dlls${compact ? " modern-inline-dlls" : ""}`}>
    <div className="advanced-option-description"><label htmlFor="advanced-dll-editor-toggle">{title}</label><p>{tx("ui-wine-dll-priorities", "priorities_applied_when_launching_the_game_open_the_editor_to_add_or_remove_dlls")}</p></div>
    <button id="advanced-dll-editor-toggle" type="button" role={compact ? undefined : "switch"} aria-expanded={compact ? editing : undefined} aria-label={tx("ui-wine-dll-priorities", "edit_wine_dll_priorities")} aria-checked={compact ? undefined : editing} disabled={disabled} className={compact ? "modern-mini-btn" : `advanced-toggle ${editing ? "enabled" : ""}`} onClick={() => { setEditing(value => !value); setDraft(""); setError(""); }}>{compact ? (editing ? (tx("ui-wine-dll-priorities", "back")) : (tx("ui-wine-dll-priorities", "edit"))) : <span />}</button>
    {editing ? <form className="advanced-dll-tags-editor" onSubmit={event => { event.preventDefault(); add(); }}>
      <div className="advanced-dll-pills">
        {detected.filter(entry => !manual.includes(entry)).map(entry => <span className="advanced-dll-pill detected" key={`auto-${entry}`} title={tx("ui-wine-dll-priorities", "detected_in_mods_managed_automatically")}>{entry}<small>mod</small></span>)}
        {manual.map(entry => <span className="advanced-dll-pill" key={entry}>{entry}<button type="button" disabled={disabled} aria-label={`${tx("ui-wine-dll-priorities", "remove")} ${entry}`} onClick={() => { if (save(manual.filter(item => item !== entry))) setError(""); }}><span aria-hidden="true">×</span></button></span>)}
        <input ref={input} maxLength={2048} aria-label={tx("ui-wine-dll-priorities", "add_wine_dll")} value={draft} disabled={disabled} onChange={event => { setDraft(event.target.value); setError(""); }} onKeyDown={event => { if(event.key === "Enter" && event.nativeEvent.isComposing) event.preventDefault(); }} placeholder="dxgi=n,b" />
      </div>
      <p>{tx("ui-wine-dll-priorities", "enter_adds_and_saves_closing_the_editor_keeps_priorities_active_dlls_marked_as_mod_are_m")}</p>
    </form> : <div className="advanced-dll-summary"><small>{tx("ui-wine-dll-priorities", "added_dlls_applied_automatically")}</small><div className="advanced-dll-pills">{displayed.map(entry => <span className="advanced-dll-pill" key={entry}>{entry}</span>)}</div>{!displayed.length && <p>{tx("ui-wine-dll-priorities", "no_dll_overrides_added")}</p>}</div>}
    <div className="advanced-dll-reset"><button type="button" className="btn btn-secondary" disabled={disabled} title={tx("ui-wine-dll-priorities", "remove_custom_priorities_and_detect_mod_dll_overrides_again")} onClick={() => { if(save([])) { setDraft(""); setError(""); onReset?.(); } }}><MenuIcon name="repair" />{tx("ui-wine-dll-priorities", "reset")}</button></div>
    {(error || detectionError) && <p role="alert" className="advanced-dll-error">{error || detectionError}</p>}
  </div>;
}

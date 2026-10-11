import { useEffect, useRef, useState } from "react";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import type { AdvancedSettings } from "../services/advancedSettings";

/** Separate editing visibility from the saved priorities applied at launch. */
export function WineDllPriorities({ settings, disabled, automatic, detectionError, onChange, onReset }: {
  settings: AdvancedSettings; disabled?: boolean; automatic?: string; detectionError?: string;
  onReset?: () => void;
  onChange: (settings: AdvancedSettings) => boolean;
}) {
  const { language } = useI18n(), pt = language === "pt";
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
  const title = pt ? "Prioridade das DLLs do Wine" : "Wine DLL priority";
  const save = (next: string[]) => onChange({ ...settings, dll_overrides: next.join(";"), options: { ...settings.options, dll_overrides: next.length > 0 } });
  const add = () => {
    const value = draft.trim();
    if (!value || disabled) return;
    if (!/^[\w.*-]+(?:,[\w.*-]+)*=(?:n|b|d|n,b|b,n)?(?:;[\w.*-]+(?:,[\w.*-]+)*=(?:n|b|d|n,b|b,n)?)*$/.test(value)) {
      setError(pt ? "Use o formato dxgi=n,b;dinput8=n,b." : "Use the format dxgi=n,b;dinput8=n,b."); return;
    }
    // Each name has one priority; adding it again updates its existing pill.
    const priorities = new Map<string, string>();
    for (const entry of [...manual, ...entries(value)]) {
      const [names, order] = entry.split("=");
      for (const name of names.split(",")) priorities.set(name.toLowerCase(), order);
    }
    if (save([...priorities].map(([name, order]) => `${name}=${order}`))) { setDraft(""); setError(""); }
  };
  return <div className="advanced-option advanced-wine-dlls">
    <div className="advanced-option-description"><label htmlFor="advanced-dll-editor-toggle">{title}</label><p>{pt ? "Prioridades aplicadas ao iniciar o jogo. Abra o editor para adicionar ou remover DLLs." : "Priorities applied when launching the game. Open the editor to add or remove DLLs."}</p></div>
    <button id="advanced-dll-editor-toggle" type="button" role="switch" aria-label={pt ? "Editar prioridades das DLLs do Wine" : "Edit Wine DLL priorities"} aria-checked={editing} disabled={disabled} className={`advanced-toggle ${editing ? "enabled" : ""}`} onClick={() => { setEditing(value => !value); setDraft(""); setError(""); }}><span /></button>
    {editing ? <form className="advanced-dll-tags-editor" onSubmit={event => { event.preventDefault(); add(); }}>
      <div className="advanced-dll-pills">
        {detected.filter(entry => !manual.includes(entry)).map(entry => <span className="advanced-dll-pill detected" key={`auto-${entry}`} title={pt ? "Detectada nos mods; gerenciada automaticamente." : "Detected in mods; managed automatically."}>{entry}<small>mod</small></span>)}
        {manual.map(entry => <span className="advanced-dll-pill" key={entry}>{entry}<button type="button" disabled={disabled} aria-label={`${pt ? "Remover" : "Remove"} ${entry}`} onClick={() => { if (save(manual.filter(item => item !== entry))) setError(""); }}><span aria-hidden="true">×</span></button></span>)}
        <input ref={input} maxLength={2048} aria-label={pt ? "Adicionar DLL do Wine" : "Add Wine DLL"} value={draft} disabled={disabled} onChange={event => { setDraft(event.target.value); setError(""); }} onKeyDown={event => { if(event.key === "Enter" && event.nativeEvent.isComposing) event.preventDefault(); }} placeholder="dxgi=n,b" />
      </div>
      <p>{pt ? "Enter adiciona e salva. Recolher o editor mantém as prioridades ativas. DLLs marcadas como mod são gerenciadas automaticamente." : "Enter adds and saves. Closing the editor keeps priorities active. DLLs marked as mod are managed automatically."}</p>
    </form> : <div className="advanced-dll-summary"><small>{pt ? "DLLs adicionadas · aplicadas automaticamente" : "Added DLLs · applied automatically"}</small><div className="advanced-dll-pills">{displayed.map(entry => <span className="advanced-dll-pill" key={entry}>{entry}</span>)}</div>{!displayed.length && <p>{pt ? "Nenhuma DLL adicionada." : "No DLL overrides added."}</p>}</div>}
    <div className="advanced-dll-reset"><button type="button" className="btn btn-secondary" disabled={disabled} title={pt ? "Remover prioridades personalizadas e detectar novamente as DLLs dos mods." : "Remove custom priorities and detect mod DLL overrides again."} onClick={() => { if(save([])) { setDraft(""); setError(""); onReset?.(); } }}><MenuIcon name="repair" />{pt ? "Redefinir" : "Reset"}</button></div>
    {(error || detectionError) && <p role="alert" className="advanced-dll-error">{error || detectionError}</p>}
  </div>;
}

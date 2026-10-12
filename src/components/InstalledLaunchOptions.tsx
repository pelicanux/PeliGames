import { useState } from "react";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";
import { NexusPopup } from "../modules/nexus/NexusPopup";
import type { GameInfo } from "./GameGrid";

export function InstalledLaunchOptions({ game, disabled, onSave, errorMessage }: {
  game: GameInfo; disabled: boolean; errorMessage: string; onSave: (options: string | null) => Promise<boolean>;
}) {
  const { language } = useI18n(), pt = language === "pt";
  const [open, setOpen] = useState(false), [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const title = pt ? "Opções de inicialização" : "Launch options";
  const automatic = (game.launch_arguments ?? []).map(arg => JSON.stringify(arg)).join(" ");
  const save = async (value: string | null) => {
    if (saving || disabled) return;
    setSaving(true);
    try { if (await onSave(value)) setOpen(false); }
    finally { setSaving(false); }
  };
  return <section className="nexus-advanced-launch-options">
    <div><h4><MenuIcon name="play" />{title}</h4><p className="nexus-hint">{pt ? "Confira e edite os argumentos e variáveis do jogo." : "Review and edit game arguments and variables."}</p></div>
    <button type="button" className="btn btn-secondary" disabled={disabled} onClick={() => { setDraft(game.advanced?.launch_options ?? automatic); setOpen(true); }}><MenuIcon name="edit" />{pt ? "Editar" : "Edit"}</button>
    {open && <NexusPopup title={title} onClose={() => { if (!saving) setOpen(false); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={saving || disabled} onClick={() => void save(null)}>{pt ? "Restaurar automáticas" : "Restore automatic"}</button>
      <button type="button" className="btn btn-primary" disabled={saving || disabled} onClick={() => void save(draft === automatic ? null : draft)}>{saving ? pt ? "Salvando…" : "Saving…" : pt ? "Salvar" : "Save"}</button>
    </>}>
      <label className="nexus-launch-options-label">{pt ? "Argumentos detectados no atalho do jogo" : "Arguments detected in the game shortcut"}</label>
      <pre className="nexus-launch-options-automatic">{automatic || (pt ? "Nenhum argumento automático adicional." : "No additional automatic arguments.")}</pre>
      <p className="nexus-hint">{pt ? "Variáveis vêm antes dos argumentos, por exemplo: MANGOHUD=1 %command% -windowed. Configure a prioridade das DLLs no campo próprio. Não são executados comandos de shell." : "Variables precede arguments, for example: MANGOHUD=1 %command% -windowed. Configure DLL priorities in their own field. Shell commands are not executed."}</p>
      <label className="nexus-launch-options-label" htmlFor="installed-launch-options">{pt ? "Opções usadas ao iniciar" : "Options used on launch"}</label>
      <textarea id="installed-launch-options" className="nexus-launch-options-input" rows={5} maxLength={4096} value={draft} disabled={saving || disabled} onChange={event => setDraft(event.target.value)} />
      <p className="nexus-hint">{pt ? "As opções personalizadas substituem os argumentos automáticos ao iniciar este jogo pela biblioteca principal." : "Custom options replace automatic arguments when launching this game from the main library."}</p>
      {errorMessage && <p role="alert" className="nexus-error">{errorMessage}</p>}
    </NexusPopup>}
  </section>;
}

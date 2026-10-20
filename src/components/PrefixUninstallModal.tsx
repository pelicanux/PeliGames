import { NexusPopup } from "../modules/nexus/NexusPopup";
import { MenuIcon } from "./MenuIcon";
import "./GameCopyDialog.css";
import { useState } from "react";

import { useI18n } from "../i18n/I18nContext";
import type { UninstallOptions } from "../services/installedLibrary";
import "../App.css";
import "./PrefixUninstallModal.css";

interface Props {
  name: string;
  prefix: string;
  busy?: boolean;
  error?: string;
  onConfirm: (options: UninstallOptions) => void;
  onCancel: () => void;
  onRemoveOnly?: () => void;
}

/** Uninstall like Heroic: the prefix and the settings/logs are separate choices;
 * the entries and their shortcuts always leave the library. */
export function PrefixUninstallModal({ name, prefix, busy = false, error, onConfirm, onCancel, onRemoveOnly }: Props) {
  const { language } = useI18n(); const pt = language !== "en";
  const [removePrefix, setRemovePrefix] = useState(false), [removeSettings, setRemoveSettings] = useState(false);
  const [removeOnly,setRemoveOnly]=useState(false);
  const title = pt ? "Desinstalar?" : "Uninstall?";
  const summary = removeOnly ? (pt ? "Somente esta entrada sai da biblioteca. O jogo, o prefixo, os saves, as configurações, os logs e os atalhos permanecem no disco." : "Only this entry leaves the library. Game files, prefix, saves, settings, logs and shortcuts stay on disk.") : removePrefix
    ? (pt ? "O prefixo será apagado permanentemente, com o jogo, os programas e os saves que estão dentro dele." : "The prefix will be permanently deleted, with the game, programs and saves inside it.")
    : (pt ? "O prefixo e o jogo continuam no disco; só saem da biblioteca." : "The prefix and the game stay on disk; they only leave the library.");
  return <NexusPopup className="game-copy-dialog prefix-uninstall-dialog" title={title} titleIcon={<MenuIcon name="trash"/>} onClose={()=>{if(!busy)onCancel();}} footer={        <>
          <button disabled={busy} onClick={onCancel}>{pt ? "Cancelar" : "Cancel"}</button>
          <button className="btn-primary" disabled={busy || (!removeOnly && !removePrefix && !removeSettings)} onClick={() => removeOnly ? onRemoveOnly?.() : onConfirm({ remove_prefix: removePrefix, remove_settings: removeSettings })}>{busy ? (pt ? "Desinstalando…" : "Uninstalling…") : removeOnly ? (pt ? "Remover da biblioteca" : "Remove from library") : (pt ? "Desinstalar" : "Uninstall")}</button>
        </>}>
        <div className="uninstall-body">
          <p className="uninstall-target"><strong>{name}</strong><br/><code>{prefix}</code></p>
          <div className="uninstall-options">
            <label className="uninstall-option"><input type="checkbox" checked={removePrefix} disabled={busy || removeOnly} onChange={event => setRemovePrefix(event.target.checked)}/><span><strong>{pt ? "Remover prefixo" : "Remove prefix"}</strong><small>{pt ? "Apaga a pasta do Wine/Proton e tudo o que foi instalado nela, inclusive saves." : "Deletes the Wine/Proton folder and everything installed in it, saves included."}</small></span></label>
            <label className="uninstall-option"><input type="checkbox" checked={removeSettings} disabled={busy || removeOnly} onChange={event => setRemoveSettings(event.target.checked)}/><span><strong>{pt ? "Apagar configurações e remover logs" : "Delete settings and remove logs"}</strong><small>{pt ? "Apaga o registro do PeliGames (installation.json) e a pasta de logs." : "Deletes the PeliGames record (installation.json) and the logs folder."}</small></span></label>
            {onRemoveOnly && <label className="uninstall-option"><input type="checkbox" checked={removeOnly} disabled={busy} onChange={event=>{setRemoveOnly(event.target.checked);setRemovePrefix(false);setRemoveSettings(false);}}/><span><strong>{pt ? "Só remover da biblioteca" : "Only remove from library"}</strong><small>{pt ? "Preserva todos os arquivos e remove somente esta entrada do PeliGames." : "Keeps all files and removes only this PeliGames entry."}</small></span></label>}
          </div>
          <p className="uninstall-summary">{summary} {!removeOnly && (pt ? "Todos os jogos que usam esse prefixo saem da biblioteca, com seus atalhos. Arquivos fora do prefixo não são apagados." : "All games using this prefix leave the library, with their shortcuts. Files outside the prefix are not deleted.")}</p>
          {error && <p className="preferences-error" role="alert">{error}</p>}
        </div>
    </NexusPopup>;
}

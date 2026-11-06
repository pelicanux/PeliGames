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
  const {t: tx} = useI18n();

  const [removePrefix, setRemovePrefix] = useState(false), [removeSettings, setRemoveSettings] = useState(false);
  const [removeOnly,setRemoveOnly]=useState(false);
  const title = tx("ui-prefix-uninstall-modal", "uninstall");
  const summary = removeOnly ? (tx("ui-prefix-uninstall-modal", "only_this_entry_leaves_the_library_game_files_prefix_saves_settings_logs_and_shortcuts_s")) : removePrefix
    ? (tx("ui-prefix-uninstall-modal", "the_prefix_will_be_permanently_deleted_with_the_game_programs_and_saves_inside_it"))
    : (tx("ui-prefix-uninstall-modal", "the_prefix_and_the_game_stay_on_disk_they_only_leave_the_library"));
  return <NexusPopup className="game-copy-dialog prefix-uninstall-dialog" title={title} titleIcon={<MenuIcon name="trash"/>} onClose={()=>{if(!busy)onCancel();}} footer={        <>
          <button disabled={busy} onClick={onCancel}>{tx("ui-prefix-uninstall-modal", "cancel")}</button>
          <button className="btn-primary" disabled={busy || (!removeOnly && !removePrefix && !removeSettings)} onClick={() => removeOnly ? onRemoveOnly?.() : onConfirm({ remove_prefix: removePrefix, remove_settings: removeSettings })}>{busy ? (tx("ui-prefix-uninstall-modal", "uninstalling")) : removeOnly ? (tx("ui-prefix-uninstall-modal", "remove_from_library")) : (removePrefix && removeSettings ? (tx("ui-prefix-uninstall-modal", "uninstall_everything")) : (tx("ui-prefix-uninstall-modal", "uninstall_2")))}</button>
        </>}>
        <div className="uninstall-body">
          <p className="uninstall-target"><strong>{name}</strong><br/><code>{prefix}</code></p>
          <div className="uninstall-options">
            <label className="uninstall-option"><input type="checkbox" checked={removePrefix} disabled={busy || removeOnly} onChange={event => setRemovePrefix(event.target.checked)}/><span><strong>{tx("ui-prefix-uninstall-modal", "remove_prefix")}</strong><small>{tx("ui-prefix-uninstall-modal", "deletes_the_wine_proton_folder_and_everything_installed_in_it_saves_included")}</small></span></label>
            <label className="uninstall-option"><input type="checkbox" checked={removeSettings} disabled={busy || removeOnly} onChange={event => setRemoveSettings(event.target.checked)}/><span><strong>{tx("ui-prefix-uninstall-modal", "delete_settings_logs_and_profiles")}</strong><small>{tx("ui-prefix-uninstall-modal", "deletes_the_game_record_and_log_folders_when_removing_the_prefix_too_also_deletes_profil")}</small></span></label>
            {onRemoveOnly && <label className="uninstall-option"><input type="checkbox" checked={removeOnly} disabled={busy} onChange={event=>{setRemoveOnly(event.target.checked);setRemovePrefix(false);setRemoveSettings(false);}}/><span><strong>{tx("ui-prefix-uninstall-modal", "only_remove_from_library")}</strong><small>{tx("ui-prefix-uninstall-modal", "keeps_all_files_and_removes_only_this_peligames_entry")}</small></span></label>}
          </div>
          <p className="uninstall-summary">{summary} {!removeOnly && (tx("ui-prefix-uninstall-modal", "all_games_using_this_prefix_leave_the_library_with_their_shortcuts_complete_cleanup_incl"))}</p>
          {error && <p className="preferences-error" role="alert">{error}</p>}
        </div>
    </NexusPopup>;
}

import { ModOptionSwitch } from "../../components/ModOptionRow";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { HoverTooltip } from "../../components/HoverTooltip";
import { NexusPopup } from "./NexusPopup";
import { useI18n } from "../../i18n/I18nContext";

type BackupSettings = { per_game: boolean; game_overrides?: Record<string, boolean> };
/** The game's recovery copy as the backend sees it (shared with the general backup settings). */
export type GameBackupStatus = { game_id: string; name: string; enabled: boolean; choice: boolean | null; has_mods: boolean; updated: number | null; size: number | null; error: string | null; updating: boolean };
export const backupDate = (seconds: number, language: string) => { return new Date(seconds * 1000).toLocaleString(language, { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" }); };
export const backupSize = (bytes: number) => bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.max(0.1, bytes / 1e6).toFixed(1)} MB`;
export function NexusGameBackup({ gameId, disabled, onError, onSaved, onStatusTextChange }: { gameId: string; disabled: boolean; onError: (error: string) => void; onSaved: () => void; onStatusTextChange?: (text: string) => void }) {

  const {t: tx,  language } = useI18n();
  const [settings, setSettings] = useState<BackupSettings | null>(null), [saving, setSaving] = useState(false), [confirmDelete, setConfirmDelete] = useState(false), [deleteError, setDeleteError] = useState("");
  const [status, setStatus] = useState<GameBackupStatus | null>(null);
  const [statusLoaded, setStatusLoaded] = useState(false), [statusReadFailed, setStatusReadFailed] = useState(false);
  const pending = useRef(false), currentGame = useRef(gameId); currentGame.current = gameId;
  useEffect(() => {
    let active = true, timer: number | undefined; setSettings(null); setStatus(null); setStatusLoaded(false); setStatusReadFailed(false); setConfirmDelete(false); setDeleteError("");
    // Asked again only while a copy is being written; otherwise on changes.
    const loadStatus = () => {
      window.clearTimeout(timer);
      void invoke<GameBackupStatus[]>("nexus_backup_status", { gameId }).then(list => {
        if (!active) return;
        const value = list[0] ?? null; setStatus(value); setStatusLoaded(true); setStatusReadFailed(false);
        if (value?.updating) timer = window.setTimeout(loadStatus, 2000);
      }).catch(() => { if (active) { setStatusReadFailed(true); setStatusLoaded(true); } });
    };
    const refresh = () => { loadStatus(); if (pending.current) return; void invoke<BackupSettings>("load_mod_backup_settings").then(value => { if (active && !pending.current) setSettings(value); }).catch(e => { if (active) onError(String(e)); }); };
    refresh(); window.addEventListener("modBackupSettingsChanged", refresh);
    return () => { active = false; window.clearTimeout(timer); window.removeEventListener("modBackupSettingsChanged", refresh); };
  }, [gameId]);
  const enabled = settings?.game_overrides?.[gameId] ?? settings?.per_game ?? false;
  const toggle = async (enabled: boolean) => {
    if (pending.current || disabled || !settings) return;
    pending.current = true; setSaving(true);
    const id = gameId;
    try {
      const value = await invoke<BackupSettings>("set_nexus_game_backup", { gameId: id, enabled });
      if (currentGame.current === id) { setSettings(value); onSaved(); }
      window.dispatchEvent(new Event("modBackupSettingsChanged"));
    } catch (e) { if (currentGame.current === id) onError(String(e)); }
    finally { pending.current = false; setSaving(false); }
  };
  const removeBackup = async () => {
    if (pending.current || disabled || !settings) return;
    pending.current = true; setSaving(true); setDeleteError("");
    const id = gameId;
    try {
      const value = await invoke<BackupSettings>("delete_nexus_game_backup", { gameId: id });
      if (currentGame.current === id) { setSettings(value); setConfirmDelete(false); onSaved(); }
    } catch (e) {
      if (currentGame.current === id) setDeleteError(String(e));
      // Deletion can fail after disabling the copy; keep the switch truthful.
      try {
        const value = await invoke<BackupSettings>("load_mod_backup_settings");
        if (currentGame.current === id) setSettings(value);
      } catch { /* Keep the original deletion error visible. */ }
    } finally {
      pending.current = false; setSaving(false);
      window.dispatchEvent(new Event("modBackupSettingsChanged"));
    }
  };
  const locked = disabled || saving || !settings;
  const when = status?.updated ? backupDate(status.updated, language) : "", size = status?.size ? ` · ${backupSize(status.size)}` : "";
  const line = !status ? "" : status.error ? (tx("ui-nexus-game-backup", "last_copy_failed", {"status_error": status.error}))
    : status.updating ? (tx("ui-nexus-game-backup", "updating_the_recovery_copy"))
    : status.enabled ? (!status.has_mods ? (tx("ui-nexus-game-backup", "the_copy_is_created_once_mods_are_installed"))
      : when ? (tx("ui-nexus-game-backup", "last_copy", {"when": when, "size": size})) : (tx("ui-nexus-game-backup", "creating_the_first_copy")))
    : when ? (tx("ui-nexus-game-backup", "old_copy_from_no_longer_updated_enable_it_to_update_or_delete_it", {"when": when, "size": size})) : "";
  const follows = status && status.choice === null ? (tx("ui-nexus-game-backup", "follows_the_general_option_in_settings_tools")) : "";
  const statusText = statusReadFailed ? tx("ui-nexus-game-backup", "could_not_read_the_backup_information")
    : [line || (statusLoaded ? tx("ui-nexus-game-backup", "no_backup_saved_for_this_game") : tx("ui-nexus-game-backup", "checking_this_game_s_backup")), follows].filter(Boolean).join("\n");
  useEffect(() => { onStatusTextChange?.(statusText); }, [statusText, onStatusTextChange]);
  return <div className="installation-field nexus-game-backup-control">
    <label><MenuIcon name="floppy" />{tx("ui-nexus-game-backup", "individual_backup")}</label>
    <div className="nexus-game-backup-actions">
      <HoverTooltip text={tx("ui-nexus-game-backup", "saves_packages_and_records_in_this_game_s_peligames_folder_this_choice_applies_only_to_t")} anchorClassName="nexus-game-backup-tooltip">
        <ModOptionSwitch label={tx("ui-nexus-game-backup", "recovery_copy_for_this_game")} enabled={enabled} disabled={locked} onToggle={() => void toggle(!enabled)} />
      </HoverTooltip>
      <HoverTooltip text={tx("ui-nexus-game-backup", "delete_this_game_s_backup")}>
        <button type="button" className="nexus-game-backup-delete" aria-label={tx("ui-nexus-game-backup", "delete_this_game_s_backup")} disabled={locked} onClick={() => { setDeleteError(""); setConfirmDelete(true); }}><MenuIcon name="trash" /></button>
      </HoverTooltip>
    </div>
    {!onStatusTextChange && <small className={`nexus-game-backup-status${statusReadFailed || status?.error ? " is-error" : status?.updated && !status.enabled ? " is-stale" : ""}`} role={statusReadFailed || status?.error ? "alert" : "status"}>{statusText}</small>}
    {confirmDelete && <NexusPopup className="nexus-remove-popup" title={tx("ui-nexus-game-backup", "delete_individual_backup")} onClose={() => { if (!pending.current) setConfirmDelete(false); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={saving} onClick={() => setConfirmDelete(false)}>{tx("ui-nexus-game-backup", "cancel")}</button>
      <button type="button" className="btn btn-primary" disabled={locked} onClick={() => void removeBackup()}><MenuIcon name="trash" />{saving ? tx("ui-nexus-game-backup", "deleting") : tx("ui-nexus-game-backup", "delete_backup")}</button>
    </>}>
      <p className="nexus-hint">{tx("ui-nexus-game-backup", "delete_the_mod_recovery_copy_in_this_game_s_folder_this_also_disables_individual_backup_")}</p>
      <p className="nexus-hint">{tx("ui-nexus-game-backup", "installed_mods_and_data_in_config_will_be_kept_this_deletion_cannot_be_undone")}</p>
      {deleteError && <p role="alert" className="nexus-error">{deleteError}</p>}
    </NexusPopup>}
  </div>;
}

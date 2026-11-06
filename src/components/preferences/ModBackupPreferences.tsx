import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ConfirmModal } from "../ConfirmModal";
import { MenuIcon } from "../MenuIcon";
import { useI18n } from "../../i18n/I18nContext";
import { backupDate, type GameBackupStatus } from "../../modules/nexus/NexusGameBackup";

type Settings = { per_game: boolean; last_error: string; synchronizing: boolean };
type Report = { restored: number; skipped: number; warnings: string[]; path: string };
export function ModBackupPreferences({ onError, onBusy, onRestored }: { onError: (error: string) => void; onBusy: (busy: boolean) => void; onRestored: () => Promise<void> }) {

  const {t: tx,  language } = useI18n();
  const [settings, setSettings] = useState<Settings | null>(null), [busy, setBusy] = useState(false), [notice, setNotice] = useState("");
  const [copies, setCopies] = useState<GameBackupStatus[]>([]);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const lock = useRef(false);
  useEffect(() => {
    let mounted = true;
    const refresh = () => {
      void invoke<Settings>("load_mod_backup_settings").then(s => { if (mounted) setSettings(s); }).catch(e => { if (mounted) onError(String(e)); });
      // The same per-game state the game panels show.
      void invoke<GameBackupStatus[]>("nexus_backup_status", { gameId: null }).then(list => { if (mounted) setCopies(list.filter(game => game.has_mods || game.updated)); }).catch(() => {});
    };
    refresh(); const timer = window.setInterval(refresh, 4000);
    window.addEventListener("modBackupSettingsChanged", refresh);
    return () => { mounted = false; window.clearInterval(timer); window.removeEventListener("modBackupSettingsChanged", refresh); };
  }, []);
  const run = async (action: () => Promise<void>) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); onBusy(true); setNotice(""); onError("");
    try { await action(); } catch (e) { onError(String(e)); }
    finally { lock.current = false; setBusy(false); onBusy(false); }
  };
  const backup = () => run(async () => {
    const folder = await open({ directory: true, multiple: false, title: tx("ui-mod-backup-preferences", "where_should_the_complete_mod_backup_be_saved") });
    if (typeof folder !== "string") return;
    const report = await invoke<Report>("create_mod_backup", { destination: folder });
    setNotice(`${tx("ui-mod-backup-preferences", "backup_saved_to")}: ${report.path}`);
  });
  const restore = () => run(async () => {
    const folder = await open({ multiple: false, filters: [{ name: "Backup Nexus", extensions: ["zip"] }], title: tx("ui-mod-backup-preferences", "select_the_nexus_backup_zip") });
    if (typeof folder !== "string") return;
    const report = await invoke<Report>("restore_mod_backup", { source: folder });
    setNotice(tx("ui-mod-backup-preferences", "games_restored_existing_games_preserved", {"report_restored": report.restored, "report_skipped": report.skipped}));
    if (report.warnings.length) onError(report.warnings.join("\n"));
    await onRestored();
    window.dispatchEvent(new Event("refreshGames"));
  });
  const recoverDisk = () => run(async () => {
    const folder = await open({ directory: true, multiple: false, title: tx("ui-mod-backup-preferences", "select_the_drive_or_game_library_folder") });
    if (typeof folder !== "string") return;
    const report = await invoke<Report>("recover_mods_from_disk", { directory: folder });
    setNotice(tx("ui-mod-backup-preferences", "nexus_games_restored_existing_games_preserved", {"report_restored": report.restored, "report_skipped": report.skipped}));
    if (report.warnings.length) onError(report.warnings.join("\n"));
    await onRestored();
    window.dispatchEvent(new Event("refreshGames"));
  });
  const deleteBackups = () => run(async () => {
    try {
      setSettings(await invoke<Settings>("delete_all_mod_backups"));
      setNotice(tx("ui-mod-backup-preferences", "internal_backups_deleted_automatic_recovery_has_been_disabled"));
    } finally {
      setConfirmDelete(false);
      window.dispatchEvent(new Event("modBackupSettingsChanged"));
    }
  });
  return <section className="mod-workspace mod-backup-workspace mod-backup-card">
    <header className="mod-intro"><h3>{tx("ui-mod-backup-preferences", "backup_and_recovery")}</h3><p>{tx("ui-mod-backup-preferences", "keep_a_copy_or_recover_an_existing_installation_accounts_are_excluded_from_backups")}</p></header>
    <div className="mod-backup-actions">
      <div data-preference-id="backup-create" className="mod-setting-row"><div className="mod-row-copy"><span className="mod-component-icon"><MenuIcon name="floppy"/></span><div><strong>{tx("ui-mod-backup-preferences", "keep_a_copy")}</strong><small>{tx("ui-mod-backup-preferences", "creates_a_zip_with_packages_profiles_and_records_for_all_modded_games")}</small></div></div><button type="button" className="mod-action-primary" disabled={busy} onClick={backup}>{tx("ui-mod-backup-preferences", "create_backup")}</button></div>
      <div data-preference-id="backup-restore" className="mod-setting-row"><div className="mod-row-copy"><span className="mod-component-icon"><MenuIcon name="download"/></span><div><strong>{tx("ui-mod-backup-preferences", "restore_from_a_backup")}</strong><small>{tx("ui-mod-backup-preferences", "imports_your_zip_already_configured_games_are_preserved")}</small></div></div><button type="button" className="mod-action-secondary" disabled={busy} onClick={restore}>{tx("ui-mod-backup-preferences", "restore")}</button></div>
      <div data-preference-id="backup-discover" className="mod-setting-row"><div className="mod-row-copy"><span className="mod-component-icon"><MenuIcon name="folderSearch"/></span><div><strong>{tx("ui-mod-backup-preferences", "find_a_previous_installation")}</strong><small>{tx("ui-mod-backup-preferences", "finds_peligames_recovery_records_in_your_game_folders")}</small></div></div><button type="button" className="mod-action-secondary" disabled={busy} onClick={recoverDisk}>{tx("ui-mod-backup-preferences", "search_drive")}</button></div>
      <div data-preference-id="backup-delete-all" className="mod-setting-row"><div className="mod-row-copy"><span className="mod-component-icon"><MenuIcon name="trash"/></span><div><strong>{tx("ui-mod-backup-preferences", "delete_all_mod_backups")}</strong><small>{tx("ui-mod-backup-preferences", "deletes_internal_recovery_copies_including_copies_preserved_after_uninstalling_games_kee")}</small></div></div><button type="button" className="mod-action-secondary" disabled={busy || !settings} onClick={()=>setConfirmDelete(true)}>{tx("ui-mod-backup-preferences", "delete_backups")}</button></div>
    </div>
    <div className="mod-backup-policy"><h4>{tx("ui-mod-backup-preferences", "per_game_recovery")}</h4>
    <label data-preference-id="backup-per-game" className="preference-switch" style={{ marginTop: 20 }}>
      <input type="checkbox" disabled={busy || !settings} checked={settings?.per_game ?? false} onChange={event => { const perGame = event.target.checked; void run(async () => { setSettings(await invoke<Settings>("set_mod_backup_settings", { perGame })); window.dispatchEvent(new Event("modBackupSettingsChanged")); }); }}/>
      <span className="preference-switch-track"/><span>{tx("ui-mod-backup-preferences", "recovery_copy_in_each_game_folder")}<small>{tx("ui-mod-backup-preferences", "keeps_a_hidden_peligames_folder_only_for_games_with_installed_nexus_mods_uses_extra_disk")}</small></span>
    </label>
    </div>
    {copies.length > 0 && <ul className="mod-backup-copies" aria-label={tx("ui-mod-backup-preferences", "recovery_copies_by_game")}>
      {copies.map(game => <li key={game.game_id} className={game.error ? "is-error" : game.updated && !game.enabled ? "is-stale" : ""}>
        <strong>{game.name}</strong>
        <small>{game.error ? (tx("ui-mod-backup-preferences", "failed", {"game_error": game.error}))
          : game.updating ? (tx("ui-mod-backup-preferences", "updating"))
          : game.enabled ? (game.updated ? (tx("ui-mod-backup-preferences", "updated", {"backupdate_game_updated_pt": backupDate(game.updated, language)})) : (tx("ui-mod-backup-preferences", "waiting_for_the_first_copy")))
          : game.updated ? (tx("ui-mod-backup-preferences", "old_copy_from_disabled", {"backupdate_game_updated_pt": backupDate(game.updated, language)})) : (tx("ui-mod-backup-preferences", "no_copy"))}
          {game.choice !== null && (tx("ui-mod-backup-preferences", "game_s_own_choice"))}</small>
      </li>)}
    </ul>}
    {settings?.synchronizing && <p role="status">{tx("ui-mod-backup-preferences", "updating_recovery_copies_on_disk")}</p>}
    {busy && <p role="status">{tx("ui-mod-backup-preferences", "processing_files")}</p>}
    {notice && <p role="status" style={{ overflowWrap: "anywhere" }}>{notice}</p>}
    {settings?.last_error && <p role="alert" className="preferences-error" style={{ whiteSpace: "pre-wrap" }}>{settings.last_error}</p>}
    {confirmDelete && <ConfirmModal title={tx("ui-mod-backup-preferences", "delete_all_mod_backups_2")} message={tx("ui-mod-backup-preferences", "this_permanently_deletes_internal_recovery_copies_for_all_games_and_disables_automatic_c")} confirmText={tx("ui-mod-backup-preferences", "delete_backups")} cancelText={tx("ui-mod-backup-preferences", "cancel")} isDanger busy={busy} onCancel={()=>{if(!busy)setConfirmDelete(false);}} onConfirm={()=>void deleteBackups()}/>}
  </section>;
}

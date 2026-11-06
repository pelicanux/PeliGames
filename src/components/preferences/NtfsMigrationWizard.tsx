import { useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../MenuIcon";
import { useI18n } from "../../i18n/I18nContext";
import { storageNameInput } from "./storageName";
import type { Volume } from "./DiskAutomountPreferences";

type Props = { volume: Volume; onBusy: (busy: boolean) => void; onDone: (mountPath: string) => Promise<void> };
export function NtfsMigrationWizard({ volume, onBusy, onDone }: Props) {
  const {t: tx} = useI18n();

  const [step, setStep] = useState(0), [backup, setBackup] = useState(false);
  const [name, setName] = useState(storageNameInput(volume.label || "Jogos"));
  const [filesystem, setFilesystem] = useState<"ext4" | "btrfs">("ext4"), [startup, setStartup] = useState(true);
  const [shared, setShared] = useState(false);
  const [confirmation, setConfirmation] = useState(""), [busy, setBusy] = useState(false), [error, setError] = useState("");
  // A failed destructive request is never retried automatically. Refresh the device list first.
  const [attempted, setAttempted] = useState(false);
  const wizardRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const body = wizardRef.current?.closest<HTMLElement>(".modal-scroll-body");
    if (body) body.scrollTop = 0;
  }, [step]);
  const confirmationWord = tx("ui-ntfs-migration-wizard", "confirm");
  const preparing = Boolean(volume.preparation_kind), wholeDisk = volume.preparation_kind === "disk", freeSpace = volume.preparation_kind === "free_space";
  const blocked = volume.format_blocked_reason ?? volume.blocked_reason;
  const valid = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,15}$/.test(name);
  const format = async () => {
    if (busy || attempted || blocked || !backup || !valid || confirmation !== confirmationWord || step !== 3) return;
    setBusy(true); setAttempted(true); setError(""); onBusy(true);
    try {
      const result = preparing ? await invoke<{mount_path:string}>("prepare_empty_storage", { request: {
        id: volume.id, preparation_kind: volume.preparation_kind, preparation_identity: volume.preparation_identity,
        parent_id: volume.parent_id, offset: volume.offset, extent_size: volume.extent_size,
        name, filesystem, mount_at_startup: startup, confirmation_device: volume.device,
        confirmation_word: confirmation, backup_confirmed: backup, shared_between_systems: shared,
      } }) : await invoke<{mount_path:string}>("format_ntfs_storage", {id:volume.id, uuid:volume.uuid, name, filesystem, mountAtStartup:startup, confirmationDevice:volume.device, confirmationWord:confirmation, backupConfirmed:backup, sharedBetweenSystems:shared});
      await onDone(result.mount_path);
    } catch (e) { setError(String(e)); } finally { setBusy(false); onBusy(false); }
  };
  return <div ref={wizardRef} className="storage-ntfs-wizard">
    <p className="storage-wizard-step" role="status">{tx("ui-ntfs-migration-wizard", "step")} {step + 1}/4</p>
    {step === 0 && <>
      <h3>{wholeDisk ? (tx("ui-ntfs-migration-wizard", "prepare_this_empty_drive_for_games")) : freeSpace ? (tx("ui-ntfs-migration-wizard", "create_a_partition_in_unallocated_space")) : tx("ui-ntfs-migration-wizard", "prepare_this_partition_for_linux_games")}</h3>
      {!preparing && <p>{tx("ui-ntfs-migration-wizard", "linux_can_recognize_ntfs_however_differences_in_permissions_links_and_filenames_may_caus")}</p>}
      {!preparing && <div className="storage-warning"><p>{tx("ui-ntfs-migration-wizard", "you_can_keep_ntfs_and_leave_this_wizard_switching_to_ext4_or_btrfs_requires_formatting_t")}</p></div>}
      {preparing && <div className="storage-warning"><p>{wholeDisk ? (tx("ui-ntfs-migration-wizard", "a_gpt_table_and_a_partition_using_the_available_drive_space_will_be_created_this_flow_on")) : freeSpace ? (tx("ui-ntfs-migration-wizard", "a_new_partition_will_be_created_only_in_this_free_space_existing_partitions_will_be_pres")) : (tx("ui-ntfs-migration-wizard", "only_the_selected_partition_will_be_formatted_other_partitions_on_the_same_drive_will_be"))}</p></div>}
      <p>{tx("ui-ntfs-migration-wizard", "ext4_and_btrfs_are_linux_filesystems_windows_does_not_normally_access_them_without_addit")}</p>
    </>}
    {step === 1 && <>
      <h3>{tx("ui-ntfs-migration-wizard", "protect_your_files_before_continuing")}</h3>
      <div className="storage-warning storage-danger-warning" role="alert"><strong>{wholeDisk ? (tx("ui-ntfs-migration-wizard", "preparation_changes_the_drive_partition_table_and_erases_any_data_on_it")) : freeSpace ? (tx("ui-ntfs-migration-wizard", "old_data_remaining_in_this_unallocated_space_may_be_erased")) : tx("ui-ntfs-migration-wizard", "formatting_will_erase_all_data_on_this_partition")}</strong><p>{preparing ? (tx("ui-ntfs-migration-wizard", "check_the_device_and_capacity_even_without_a_recognized_filesystem_recoverable_old_data_")) : tx("ui-ntfs-migration-wizard", "games_documents_photos_and_all_other_files_will_be_lost_if_it_contains_windows_that_inst")}</p></div>
      <label className="storage-backup-check"><input type="checkbox" checked={backup} onChange={event => setBackup(event.target.checked)}/><span><strong>{tx("ui-ntfs-migration-wizard", "confirm_2")}</strong><small>{preparing ? (tx("ui-ntfs-migration-wizard", "i_verified_the_device_and_confirmed_this_space_is_empty_or_backed_up_i_authorize_its_pre")) : tx("ui-ntfs-migration-wizard", "i_verified_my_backup_on_another_drive_and_understand_that_all_data_on_this_partition_wil")}</small></span></label>
    </>}
    {step === 2 && <div className="storage-mount-form">
      <h3>{tx("ui-ntfs-migration-wizard", "choose_the_filesystem_and_name")}</h3>
      <fieldset className="storage-filesystem-choices"><legend>{tx("ui-ntfs-migration-wizard", "filesystem")}</legend>
        <label><input type="radio" name="storage-format" value="ext4" checked={filesystem === "ext4"} onChange={() => setFilesystem("ext4")}/><span><strong>ext4</strong><small>{tx("ui-ntfs-migration-wizard", "a_simple_and_reliable_choice_for_a_game_library")}</small></span></label>
        <label><input type="radio" name="storage-format" value="btrfs" checked={filesystem === "btrfs"} onChange={() => setFilesystem("btrfs")}/><span><strong>Btrfs</strong><small>{tx("ui-ntfs-migration-wizard", "linux_filesystem_with_features_such_as_snapshots_and_compression_configured_separately")}</small></span></label>
      </fieldset>
      <label htmlFor="ntfs-storage-name">{tx("ui-ntfs-migration-wizard", "storage_name")}</label>
      <input id="ntfs-storage-name" value={name} maxLength={16} onChange={event => setName(storageNameInput(event.target.value))} autoComplete="off" spellCheck={false} aria-describedby="ntfs-name-hint"/>
      <p id="ntfs-name-hint">{tx("ui-ntfs-migration-wizard", "use_up_to_16_characters_starting_with_a_letter_or_number_allowed_unaccented_letters_numb")}</p>
      <p>{tx("ui-ntfs-migration-wizard", "destination")} <strong className="storage-mount-path">/mnt/{name || "…"}</strong></p>
      <label className="preference-switch"><input type="checkbox" checked={startup} onChange={event => setStartup(event.target.checked)}/><span className="preference-switch-track"/><span>{tx("ui-ntfs-migration-wizard", "mount_at_system_startup")}</span></label>
      <p>{tx("ui-ntfs-migration-wizard", "the_partition_is_mounted_when_complete_even_if_startup_mounting_is_disabled")}</p>
      <label className="preference-switch"><input type="checkbox" checked={shared} onChange={event => setShared(event.target.checked)}/><span className="preference-switch-track"/><span>{tx("ui-ntfs-migration-wizard", "use_in_more_than_one_linux_system")}</span></label>
      <p>{tx("ui-ntfs-migration-wizard", "so_another_linux_installed_on_this_computer_ubuntu_cachyos_can_also_write_to_the_drive_a")}</p>
    </div>}
    {step === 3 && <>
      <h3>{wholeDisk ? (tx("ui-ntfs-migration-wizard", "review_the_entire_drive_before_preparing")) : freeSpace ? (tx("ui-ntfs-migration-wizard", "review_unallocated_space_before_creating_the_partition")) : tx("ui-ntfs-migration-wizard", "review_the_partition_before_erasing")}</h3>
      <dl className="storage-format-review"><dt>{tx("ui-ntfs-migration-wizard", "device")}</dt><dd>{volume.device}</dd>{preparing && <><dt>{tx("ui-ntfs-migration-wizard", "operation")}</dt><dd>{wholeDisk ? (tx("ui-ntfs-migration-wizard", "gpt_and_one_partition_on_the_empty_drive")) : freeSpace ? (tx("ui-ntfs-migration-wizard", "new_partition_only_in_unallocated_space")) : (tx("ui-ntfs-migration-wizard", "format_only_this_partition"))}</dd></>}<dt>{tx("ui-ntfs-migration-wizard", "capacity")}</dt><dd>{(volume.size / 1024 ** 3).toFixed(1)} GiB</dd><dt>{tx("ui-ntfs-migration-wizard", "filesystem_2")}</dt><dd>{preparing ? (wholeDisk ? "GPT + " : freeSpace ? (tx("ui-ntfs-migration-wizard", "new_partition")) : "") : "NTFS → "}{filesystem}</dd><dt>{tx("ui-ntfs-migration-wizard", "name")}</dt><dd>{name}</dd><dt>{tx("ui-ntfs-migration-wizard", "destination_2")}</dt><dd>/mnt/{name}</dd><dt>{tx("ui-ntfs-migration-wizard", "mount_at_startup")}</dt><dd>{startup ? (tx("ui-ntfs-migration-wizard", "yes")) : (tx("ui-ntfs-migration-wizard", "no"))}</dd><dt>{tx("ui-ntfs-migration-wizard", "more_than_one_linux_system")}</dt><dd>{shared ? (tx("ui-ntfs-migration-wizard", "yes")) : (tx("ui-ntfs-migration-wizard", "no"))}</dd></dl>
      <div className="storage-warning storage-danger-warning"><strong>{wholeDisk ? (tx("ui-ntfs-migration-wizard", "the_drive_above_will_be_prepared_and_formatted")) : freeSpace ? (tx("ui-ntfs-migration-wizard", "only_the_selected_unallocated_space_will_be_used_existing_partitions_will_be_preserved")) : tx("ui-ntfs-migration-wizard", "all_data_on_the_partition_above_will_be_erased")}</strong><p>{tx("ui-ntfs-migration-wizard", "close_games_and_files_on_this_drive_the_system_requests_administrator_authorization_when")}</p></div>
      <div className="storage-mount-form"><label htmlFor="storage-format-confirm">{tx("ui-ntfs-migration-wizard", "type")}<strong className="storage-confirmation-word">“{confirmationWord}”</strong>{wholeDisk ? (tx("ui-ntfs-migration-wizard", "to_authorize_preparing_and_formatting_this_entire_drive")) : freeSpace ? (tx("ui-ntfs-migration-wizard", "to_authorize_creating_and_formatting_a_new_partition_in_this_free_space")) : tx("ui-ntfs-migration-wizard", "to_authorize_erasing_this_partition")}</label><input id="storage-format-confirm" value={confirmation} disabled={busy || attempted} onChange={event => setConfirmation(event.target.value)} aria-label={wholeDisk ? (tx("ui-ntfs-migration-wizard", "type_confirm_to_prepare_and_format_the_entire_drive")) : freeSpace ? (tx("ui-ntfs-migration-wizard", "type_confirm_to_create_a_partition_in_unallocated_space")) : tx("ui-ntfs-migration-wizard", "type_confirm_to_erase_the_partition")} autoComplete="off" spellCheck={false}/></div>
    </>}
    {blocked && <div className="storage-warning" role="alert">{blocked}</div>}
    {error && <div className="preferences-error" role="alert">{error}<p>{tx("ui-ntfs-migration-wizard", "close_this_wizard_and_refresh_drives_to_inspect_the_current_state_before_making_any_furt")}</p></div>}
    {busy && <p role="status" className="storage-wizard-progress"><MenuIcon name="repair"/>{tx("ui-ntfs-migration-wizard", "waiting_for_authorization_or_preparing_the_partition")}</p>}
    <div className="storage-wizard-actions">
      {step > 0 && <button type="button" className="preference-outline" disabled={busy || attempted} onClick={() => { setStep(step - 1); setConfirmation(""); }}>{tx("ui-ntfs-migration-wizard", "back")}</button>}
      {step < 3 ? <button type="button" className="preference-outline" disabled={Boolean(blocked) || (step === 1 && !backup) || (step === 2 && !valid)} onClick={() => setStep(step + 1)}>{step === 0 ? (tx("ui-ntfs-migration-wizard", "yes_prepare_for_linux")) : (tx("ui-ntfs-migration-wizard", "next"))}</button> : <button type="button" className="btn btn-danger" disabled={busy || attempted || Boolean(blocked) || !backup || !valid || confirmation !== confirmationWord} onClick={() => void format()}>{preparing ? (tx("ui-ntfs-migration-wizard", "prepare_and_format")) : tx("ui-ntfs-migration-wizard", "format_and_erase_this_partition")}</button>}
    </div>
  </div>;
}

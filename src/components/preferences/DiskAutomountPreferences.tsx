import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "../ModalSurface";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nContext";
import { MenuIcon } from "../MenuIcon";
import { ConfirmModal } from "../ConfirmModal";
import { ResultModal } from "../ResultModal";
import { notifyError, notifyInfo } from "../../services/notifications";

import { storageNameInput } from "./storageName";
import { NtfsMigrationWizard } from "./NtfsMigrationWizard";

type StorageOperation = "mount" | "unmount" | "remount";
type FstabEntry = { key: string; source: string; mount_path: string; line: string; managed: boolean; automatic: boolean };
export type Volume = { id: string; uuid: string; device: string; label: string; partition_name: string; filesystem: string; size: number; mounts: string[]; mount_at_startup: boolean; configured_path: string; blocked_reason: string; format_blocked_reason?: string; operations_blocked_reason?: string; preparation_kind?: "disk" | "partition" | "free_space"; preparation_identity?: Record<string, unknown>; parent_id?: string; offset?: number; extent_size?: number; shared_between_systems?: boolean };
export function DiskAutomountPreferences({ onBusy, onDialogChange }: { onBusy: (busy: boolean) => void; onDialogChange: (open: boolean) => void }) {
  const {t: tx} = useI18n();

  const [volumes, setVolumes] = useState<Volume[]>([]), [selected, setSelected] = useState("");
  const [name, setName] = useState(""), [startup, setStartup] = useState(true), [shared, setShared] = useState(false);
  const [loading, setLoading] = useState(true), [saving, setSaving] = useState(false);
  const [error, setError] = useState(""), [success, setSuccess] = useState("");
  const [confirm, setConfirm] = useState<"save" | null>(null);
  const [fstabEntries, setFstabEntries] = useState<FstabEntry[]>([]);
  const [removeEntry, setRemoveEntry] = useState<FstabEntry | null>(null);
  const [operationFailure, setOperationFailure] = useState<{ title: string; message: string } | null>(null);
  const confirmationOpen = confirm !== null || operationFailure !== null || removeEntry !== null;
  const operationLock = useRef(false);
  const nameRef = useRef<HTMLInputElement>(null);
  const editorRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const volume = volumes.find(item => item.id === selected);
  const valid = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,15}$/.test(name);
  useEffect(() => { onBusy(saving || confirmationOpen || Boolean(volume)); return () => onBusy(false); }, [saving, confirmationOpen, volume, onBusy]);
  useEffect(() => { if (volume) { if (!volume.blocked_reason) nameRef.current?.focus(); else editorRef.current?.focus(); } }, [selected]);
  const closeEditor = () => { if (!saving && !confirmationOpen) { setSelected(""); setError(""); triggerRef.current?.focus(); } };
  useEffect(() => { onDialogChange(Boolean(volume) || confirmationOpen); return () => onDialogChange(false); }, [volume, confirmationOpen, onDialogChange]);
  useEffect(() => {
    if (!volume && !confirmationOpen) return;
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Tab" && !confirmationOpen) {
        const controls = Array.from(editorRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),[tabindex="0"]') ?? []).filter(item => item.getClientRects().length > 0);
        const first = controls[0], last = controls[controls.length - 1];
        if (!first) { event.preventDefault(); return; }
        if (event.shiftKey && (document.activeElement === first || document.activeElement === editorRef.current)) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
      if (event.key === "Escape") { event.stopImmediatePropagation(); if (!saving) { if (removeEntry) setRemoveEntry(null); else if (operationFailure) setOperationFailure(null); else if (confirm) setConfirm(null); else closeEditor(); } }
    };
    window.addEventListener("keydown", escape, true);
    return () => window.removeEventListener("keydown", escape, true);
  }, [volume, saving, confirm, operationFailure, removeEntry]);
  const refresh = async () => {
    setLoading(true); setError("");
    const [disks, entries] = await Promise.allSettled([invoke<Volume[]>("list_storage_volumes"), invoke<FstabEntry[]>("list_storage_fstab_entries")]);
    if (disks.status === "fulfilled") { setVolumes(disks.value); setSelected(""); setName(""); }
    else setError(String(disks.reason));
    if (entries.status === "fulfilled") setFstabEntries(entries.value);
    else { setFstabEntries([]); notifyError("fstab", entries.reason); }
    setLoading(false);
  };
  useEffect(() => { void refresh(); }, []);
  const choose = (item: Volume) => {
    setSelected(item.id); setName(storageNameInput(item.label || item.partition_name || ""));
    setStartup(item.mount_at_startup); setShared(Boolean(item.shared_between_systems)); setSuccess(""); setError("");

  };
  const save = async () => {
    if (!volume || !valid || volume.blocked_reason || saving) return;
    setSaving(true); setError(""); setSuccess("");
    try {
      const result = await invoke<{mount_path:string}>("save_storage_automount", {id:volume.id, uuid:volume.uuid, name, mountAtStartup:startup, sharedBetweenSystems:shared && !volume.shared_between_systems});
      setSuccess(tx("ui-disk-automount-preferences", "settings_saved_volume_mounted_at", {"result_mount_path": result.mount_path}));
      await refresh();
    } catch (e) { setError(String(e)); } finally { setSaving(false); setConfirm(null); }
  };
  const operationNames = { mount: tx("ui-disk-automount-preferences", "mount"), unmount: tx("ui-disk-automount-preferences", "eject"), remount: tx("ui-disk-automount-preferences", "remount") };
  const entryName = (entry: FstabEntry) => {
    const disk = volumes.find(item => entry.source === `UUID=${item.uuid}` || entry.source === `/dev/disk/by-uuid/${item.uuid}` || entry.source === `LABEL=${item.label}` || entry.source === item.device);
    return { name: disk?.label || disk?.partition_name || entry.mount_path.split("/").filter(Boolean).pop() || entry.mount_path, connected: Boolean(disk) };
  };
  const removeFstab = async () => {
    if (!removeEntry || operationLock.current || saving) return;
    operationLock.current = true; setSaving(true);
    try {
      await invoke("remove_storage_fstab_entry", { key: removeEntry.key });
      notifyInfo(tx("ui-disk-automount-preferences", "automatic_mounting"), tx("ui-disk-automount-preferences", "entry_removed_from_fstab_this_configuration_will_no_longer_mount_the_drive_on_startup_it"));
    } catch (error) { notifyError(tx("ui-disk-automount-preferences", "remove_automatic_mounting"), error); }
    finally { await refresh(); setSaving(false); operationLock.current = false; setRemoveEntry(null); triggerRef.current?.focus(); }
  };
  const requestOperation = async (item: Volume, operation: StorageOperation) => {
    if (operationLock.current || saving || confirmationOpen) return;
    operationLock.current = true;
    setSaving(true); setError(""); setSuccess("");
    try {
      await invoke("manage_storage_volume", {id:item.id, uuid:item.uuid, operation});
      setSuccess(operation === "unmount" ? (tx("ui-disk-automount-preferences", "partition_unmounted_automatic_mounting_settings_were_preserved")) : (tx("ui-disk-automount-preferences", "partition_mounted_at_the_configured_path")));
      await refresh();
    } catch (e) {
      setOperationFailure({ title: tx("ui-disk-automount-preferences", "mount_confirmation_4", {value1: (operation === "unmount" ? tx("ui-disk-automount-preferences", "mount_confirmation_1") : (operation === "remount" ? tx("ui-disk-automount-preferences", "mount_confirmation_2") : tx("ui-disk-automount-preferences", "mount_confirmation_3")))}), message: `${item.device}\n${String(e)}` });
    } finally { setSaving(false); operationLock.current = false; }
  };
  return <><h2>{tx("ui-disk-automount-preferences", "automatic_drive_mounting")}</h2>
    <p>{tx("ui-disk-automount-preferences", "choose_a_storage_partition_one_name_sets_the_filesystem_label_gpt_partition_name_and_fol")}</p>
    {import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window && <p className="storage-warning">{tx("ui-disk-automount-preferences", "demo_the_drives_below_are_examples_no_storage_is_changed_in_the_preview")}</p>}
    <section className="preference-section first storage-volume-section">
      <button type="button" className="preference-outline" disabled={loading || saving || confirmationOpen} onClick={() => void refresh()}><MenuIcon name="repair"/>{loading ? (tx("ui-disk-automount-preferences", "finding_drives")) : (tx("ui-disk-automount-preferences", "refresh_drives"))}</button>
      <div className="preference-choice-list storage-volume-list" aria-label={tx("ui-disk-automount-preferences", "available_partitions")}>
        {volumes.map(item => {
          const mounted = item.mounts.length > 0, configured = item.configured_path.startsWith("/mnt/");
          const blocked = Boolean(item.operations_blocked_reason ?? item.blocked_reason);
          return <div className="storage-volume-card" key={item.id}>
            <button type="button" disabled={saving || confirmationOpen} className={`preference-choice storage-volume-select${item.id === selected ? " selected" : ""}`} aria-pressed={item.id === selected} aria-haspopup="dialog" onClick={event => { triggerRef.current = event.currentTarget; choose(item); }}><span><strong>{item.label || item.partition_name || (item.preparation_kind === "free_space" ? (tx("ui-disk-automount-preferences", "unallocated_space")) : item.device)}</strong><small>{item.device} · {item.filesystem || (item.preparation_kind === "free_space" ? (tx("ui-disk-automount-preferences", "free_space")) : tx("ui-disk-automount-preferences", "no_filesystem"))}</small><small className="storage-volume-capacity">{tx("ui-disk-automount-preferences", "capacity")} {(item.size / 1024 ** 3).toFixed(1)} GiB</small><small>{mounted ? item.mounts.join(", ") : (tx("ui-disk-automount-preferences", "not_mounted"))}</small></span></button>
            <div className="storage-volume-card-actions">
              {mounted || !configured
                ? <button type="button" className="preference-outline" title={operationNames.unmount} aria-label={`${operationNames.unmount} · ${item.label || item.device}`} disabled={saving || confirmationOpen || blocked || !mounted} onClick={() => void requestOperation(item, "unmount")}><MenuIcon name="eject"/></button>
                : <button type="button" className="preference-outline" title={operationNames.mount} aria-label={`${operationNames.mount} · ${item.label || item.device}`} disabled={saving || confirmationOpen || blocked} onClick={() => void requestOperation(item, "mount")}><MenuIcon name="folder"/></button>}
              <button type="button" className="preference-outline" title={operationNames.remount} aria-label={`${operationNames.remount} · ${item.label || item.device}`} disabled={saving || confirmationOpen || blocked || !mounted || !configured} onClick={() => void requestOperation(item, "remount")}><MenuIcon name="repair"/></button>
            </div>
          </div>;
        })}
      </div>
      {!loading && !error && !volumes.length && <p>{tx("ui-disk-automount-preferences", "no_storage_partitions_found")}</p>}
    </section>
    <div className="storage-fstab" aria-label={tx("ui-disk-automount-preferences", "fstab_mount_entries")}>
      <h3>{tx("ui-disk-automount-preferences", "entries_in_etc_fstab")}</h3>
      <p>{tx("ui-disk-automount-preferences", "data_mounts_under_mnt_the_x_removes_only_that_line_s_configuration_it_does_not_eject_the")}</p>
      {fstabEntries.map(entry => {
        const disk = entryName(entry);
        return <div className="storage-fstab-entry" key={entry.key}>
          <div><div className="storage-fstab-entry-heading"><strong>{disk.name}</strong><small>{entry.managed ? (tx("ui-disk-automount-preferences", "managed_by_peligames")) : (tx("ui-disk-automount-preferences", "existing_entry"))} · {entry.automatic ? (tx("ui-disk-automount-preferences", "mount_at_startup")) : (tx("ui-disk-automount-preferences", "startup_disabled_noauto"))}{!disk.connected && (tx("ui-disk-automount-preferences", "drive_not_detected"))}</small></div><code>{entry.line}</code></div>
          <button type="button" className="storage-fstab-remove" disabled={saving || loading || confirmationOpen || Boolean(volume)} title={tx("ui-disk-automount-preferences", "remove_entry", {"disk_name": disk.name})} aria-label={tx("ui-disk-automount-preferences", "remove_entry", {"disk_name": disk.name})} onClick={event => { triggerRef.current = event.currentTarget; setRemoveEntry(entry); }}><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg></button>
        </div>;
      })}
      {!loading && !fstabEntries.length && <p>{tx("ui-disk-automount-preferences", "no_supported_data_entries_under_mnt")}</p>}
    </div>
    {volume && createPortal(<div className="modal-overlay storage-editor-overlay">
      <ModalSurface ref={editorRef} tabIndex={-1} className="modal-content dialog-glass storage-editor-modal" role="dialog" aria-modal="true" aria-label={tx("ui-disk-automount-preferences", "edit_storage")} onDismiss={closeEditor} closeDisabled={saving || confirmationOpen} header={<h2><MenuIcon name="eject"/>{tx("ui-disk-automount-preferences", "edit_storage")}</h2>}>
      <div className="storage-editor-content"><p className="storage-editor-device"><strong>{volume.label || volume.partition_name || volume.device}</strong> · {volume.device} · {volume.filesystem || (tx("ui-disk-automount-preferences", "no_filesystem"))}</p>
      <section className="storage-mount-form">
      {(volume.filesystem.startsWith("ntfs") || volume.preparation_kind) ? <NtfsMigrationWizard key={volume.id} volume={volume} onBusy={setSaving} onDone={async mountPath => { setSuccess(tx("ui-disk-automount-preferences", "partition_formatted_and_mounted_at", {"mountpath": mountPath})); await refresh(); }}/> : volume.blocked_reason ? <div className="storage-warning" role="status">{volume.blocked_reason}</div> : <>
        <label htmlFor="storage-name">{tx("ui-disk-automount-preferences", "storage_name")}</label>
        <input ref={nameRef} id="storage-name" value={name} maxLength={16} disabled={saving || confirmationOpen} onChange={event => setName(storageNameInput(event.target.value))} autoComplete="off" spellCheck={false} aria-describedby="storage-name-hint"/>
        <p id="storage-name-hint">{tx("ui-disk-automount-preferences", "use_up_to_16_characters_and_start_the_name_with_a_letter_or_number_allowed_unaccented_le")}</p>
        <p>{tx("ui-disk-automount-preferences", "mount_folder")} <strong className="storage-mount-path">/mnt/{name || "…"}</strong></p>
        <label className="preference-switch"><input type="checkbox" checked={startup} disabled={saving || confirmationOpen} onChange={event => setStartup(event.target.checked)}/><span className="preference-switch-track"/><span>{tx("ui-disk-automount-preferences", "mount_at_system_startup")}</span></label>
        <label className="preference-switch"><input type="checkbox" checked={shared} disabled={saving || confirmationOpen || volume.shared_between_systems} onChange={event => setShared(event.target.checked)}/><span className="preference-switch-track"/><span>{tx("ui-disk-automount-preferences", "use_in_more_than_one_linux_system")}</span></label>
        <p>{volume.shared_between_systems ? (tx("ui-disk-automount-preferences", "this_storage_is_already_open_to_other_linux_systems_any_account_can_write_to_it")) : tx("ui-disk-automount-preferences", "so_another_linux_installed_on_this_computer_can_write_here_too_on_save_every_current_fil")}</p>
        <p>{tx("ui-disk-automount-preferences", "uses_uuid_to_identify_the_partition_and_nofail_so_a_disconnected_drive_does_not_block_st")}{!shared || volume.shared_between_systems ? (tx("ui-disk-automount-preferences", "existing_file_permissions_are_preserved")) : ""}</p>
        <button type="button" className="preference-outline" disabled={!valid || saving || confirmationOpen} onClick={() => setConfirm("save")}><MenuIcon name="floppy"/>{saving ? (tx("ui-disk-automount-preferences", "applying")) : (tx("ui-disk-automount-preferences", "save_automatic_mounting"))}</button>
      </>}
      </section>
      {error && <div className="preferences-error" role="alert">{error}</div>}
      {!volume.filesystem.startsWith("ntfs") && <button type="button" className="preference-outline storage-editor-cancel" disabled={saving || confirmationOpen} onClick={closeEditor}>{tx("ui-disk-automount-preferences", "close")}</button>}
      </div></ModalSurface>
    </div>, document.body)}
    {error && !volume && <div className="preferences-error" role="alert">{error}</div>}
    {success && <p role="status">{success}</p>}
    {confirm === "save" && volume && <ConfirmModal title={tx("ui-disk-automount-preferences", "apply_automatic_mounting")} message={tx("ui-disk-automount-preferences", "mount_confirmation_8", {value1: volume.device, value2: name, value3: name, value4: (startup ? tx("ui-disk-automount-preferences", "mount_confirmation_5") : tx("ui-disk-automount-preferences", "mount_confirmation_6")), value5: (shared && !volume.shared_between_systems ? tx("ui-disk-automount-preferences", "mount_confirmation_7") : "")})} confirmText={tx("ui-disk-automount-preferences", "authorize_and_save")} cancelText={tx("ui-disk-automount-preferences", "cancel")} busy={saving} onConfirm={() => void save()} onCancel={() => { if (!saving) setConfirm(null); }}/>}
    {removeEntry && <ConfirmModal title={tx("ui-disk-automount-preferences", "remove_fstab_entry")}
      message={tx("ui-disk-automount-preferences", "mount_confirmation_12", {value1: entryName(removeEntry).name, value2: removeEntry.mount_path, value3: removeEntry.line, value4: (removeEntry.automatic ? tx("ui-disk-automount-preferences", "mount_confirmation_9") : tx("ui-disk-automount-preferences", "mount_confirmation_10")), value5: (!removeEntry.managed ? tx("ui-disk-automount-preferences", "mount_confirmation_11") : "")})}
      confirmText={tx("ui-disk-automount-preferences", "authorize_and_remove")} cancelText={tx("ui-disk-automount-preferences", "cancel")} busy={saving} onConfirm={() => void removeFstab()} onCancel={() => { if (!saving) { setRemoveEntry(null); triggerRef.current?.focus(); } }}/>}
    {operationFailure && <ResultModal title={operationFailure.title} message={operationFailure.message} type="error" logs="" showLogButton={false} onClose={() => setOperationFailure(null)}/>}</>;
}

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
  const { language } = useI18n(); const pt = language === "pt";
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
      setSuccess(pt ? `Configuração salva. Volume montado em ${result.mount_path}.` : `Settings saved. Volume mounted at ${result.mount_path}.`);
      await refresh();
    } catch (e) { setError(String(e)); } finally { setSaving(false); setConfirm(null); }
  };
  const operationNames = { mount: pt ? "Montar" : "Mount", unmount: pt ? "Ejetar" : "Eject", remount: pt ? "Remontar" : "Remount" };
  const entryName = (entry: FstabEntry) => {
    const disk = volumes.find(item => entry.source === `UUID=${item.uuid}` || entry.source === `/dev/disk/by-uuid/${item.uuid}` || entry.source === `LABEL=${item.label}` || entry.source === item.device);
    return { name: disk?.label || disk?.partition_name || entry.mount_path.split("/").filter(Boolean).pop() || entry.mount_path, connected: Boolean(disk) };
  };
  const removeFstab = async () => {
    if (!removeEntry || operationLock.current || saving) return;
    operationLock.current = true; setSaving(true);
    try {
      await invoke("remove_storage_fstab_entry", { key: removeEntry.key });
      notifyInfo(pt ? "Montagem automática" : "Automatic mounting", pt ? "Entrada removida do fstab. O disco não será montado por essa configuração na próxima inicialização. A montagem atual e os arquivos foram preservados." : "Entry removed from fstab. This configuration will no longer mount the drive on startup. Its current mount and files were preserved.");
    } catch (error) { notifyError(pt ? "Remover montagem automática" : "Remove automatic mounting", error); }
    finally { await refresh(); setSaving(false); operationLock.current = false; setRemoveEntry(null); triggerRef.current?.focus(); }
  };
  const requestOperation = async (item: Volume, operation: StorageOperation) => {
    if (operationLock.current || saving || confirmationOpen) return;
    operationLock.current = true;
    setSaving(true); setError(""); setSuccess("");
    try {
      await invoke("manage_storage_volume", {id:item.id, uuid:item.uuid, operation});
      setSuccess(operation === "unmount" ? (pt ? "Partição desmontada. A configuração de automontagem foi preservada." : "Partition unmounted. Automatic mounting settings were preserved.") : (pt ? "Partição montada no caminho configurado." : "Partition mounted at the configured path."));
      await refresh();
    } catch (e) {
      setOperationFailure({ title: pt ? `Não foi possível ${operation === "unmount" ? "ejetar" : operation === "remount" ? "remontar" : "montar"} o armazenamento` : `Could not ${operation === "unmount" ? "eject" : operation === "remount" ? "remount" : "mount"} storage`, message: `${item.device}\n${String(e)}` });
    } finally { setSaving(false); operationLock.current = false; }
  };
  return <><h2>{pt ? "Automontagem de discos" : "Automatic drive mounting"}</h2>
    <p>{pt ? "Escolha a partição de armazenamento. Um único nome define o label, o nome da partição GPT e a pasta em /mnt." : "Choose a storage partition. One name sets the filesystem label, GPT partition name and folder under /mnt."}</p>
    {import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window && <p className="storage-warning">{pt ? "Demonstração: os discos abaixo são exemplos. Nenhum armazenamento será alterado na preview." : "Demo: the drives below are examples. No storage is changed in the preview."}</p>}
    <section className="preference-section first storage-volume-section">
      <button type="button" className="preference-outline" disabled={loading || saving || confirmationOpen} onClick={() => void refresh()}><MenuIcon name="repair"/>{loading ? (pt ? "Buscando discos…" : "Finding drives…") : (pt ? "Atualizar discos" : "Refresh drives")}</button>
      <div className="preference-choice-list storage-volume-list" aria-label={pt ? "Partições disponíveis" : "Available partitions"}>
        {volumes.map(item => {
          const mounted = item.mounts.length > 0, configured = item.configured_path.startsWith("/mnt/");
          const blocked = Boolean(item.operations_blocked_reason ?? item.blocked_reason);
          return <div className="storage-volume-card" key={item.id}>
            <button type="button" disabled={saving || confirmationOpen} className={`preference-choice storage-volume-select${item.id === selected ? " selected" : ""}`} aria-pressed={item.id === selected} aria-haspopup="dialog" onClick={event => { triggerRef.current = event.currentTarget; choose(item); }}><span><strong>{item.label || item.partition_name || (item.preparation_kind === "free_space" ? (pt ? "Espaço não particionado" : "Unallocated space") : item.device)}</strong><small>{item.device} · {item.filesystem || (item.preparation_kind === "free_space" ? (pt ? "Espaço livre" : "Free space") : pt ? "Sem sistema de arquivos" : "No filesystem")}</small><small className="storage-volume-capacity">{pt ? "Capacidade:" : "Capacity:"} {(item.size / 1024 ** 3).toFixed(1)} GiB</small><small>{mounted ? item.mounts.join(", ") : (pt ? "Não montado" : "Not mounted")}</small></span></button>
            <div className="storage-volume-card-actions">
              {mounted || !configured
                ? <button type="button" className="preference-outline" title={operationNames.unmount} aria-label={`${operationNames.unmount} · ${item.label || item.device}`} disabled={saving || confirmationOpen || blocked || !mounted} onClick={() => void requestOperation(item, "unmount")}><MenuIcon name="eject"/></button>
                : <button type="button" className="preference-outline" title={operationNames.mount} aria-label={`${operationNames.mount} · ${item.label || item.device}`} disabled={saving || confirmationOpen || blocked} onClick={() => void requestOperation(item, "mount")}><MenuIcon name="folder"/></button>}
              <button type="button" className="preference-outline" title={operationNames.remount} aria-label={`${operationNames.remount} · ${item.label || item.device}`} disabled={saving || confirmationOpen || blocked || !mounted || !configured} onClick={() => void requestOperation(item, "remount")}><MenuIcon name="repair"/></button>
            </div>
          </div>;
        })}
      </div>
      {!loading && !error && !volumes.length && <p>{pt ? "Nenhuma partição de armazenamento encontrada." : "No storage partitions found."}</p>}
    </section>
    <div className="storage-fstab" aria-label={pt ? "Entradas de montagem no fstab" : "fstab mount entries"}>
      <h3>{pt ? "Entradas em /etc/fstab" : "Entries in /etc/fstab"}</h3>
      <p>{pt ? "Montagens de dados em /mnt. O X remove somente a configuração dessa linha; não ejeta o disco nem apaga seus arquivos." : "Data mounts under /mnt. The X removes only that line's configuration; it does not eject the drive or delete its files."}</p>
      {fstabEntries.map(entry => {
        const disk = entryName(entry);
        return <div className="storage-fstab-entry" key={entry.key}>
          <div><div className="storage-fstab-entry-heading"><strong>{disk.name}</strong><small>{entry.managed ? (pt ? "Gerenciado pelo PeliGames" : "Managed by PeliGames") : (pt ? "Entrada existente" : "Existing entry")} · {entry.automatic ? (pt ? "Montagem na inicialização" : "Mount at startup") : (pt ? "Inicialização desativada (noauto)" : "Startup disabled (noauto)")}{!disk.connected && (pt ? " · Disco não detectado" : " · Drive not detected")}</small></div><code>{entry.line}</code></div>
          <button type="button" className="storage-fstab-remove" disabled={saving || loading || confirmationOpen || Boolean(volume)} title={pt ? `Remover entrada de ${disk.name}` : `Remove ${disk.name} entry`} aria-label={pt ? `Remover entrada de ${disk.name}` : `Remove ${disk.name} entry`} onClick={event => { triggerRef.current = event.currentTarget; setRemoveEntry(entry); }}><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg></button>
        </div>;
      })}
      {!loading && !fstabEntries.length && <p>{pt ? "Nenhuma entrada de dados compatível em /mnt." : "No supported data entries under /mnt."}</p>}
    </div>
    {volume && createPortal(<div className="modal-overlay storage-editor-overlay">
      <ModalSurface ref={editorRef} tabIndex={-1} className="modal-content dialog-glass storage-editor-modal" role="dialog" aria-modal="true" aria-label={pt ? "Editar armazenamento" : "Edit storage"} onDismiss={closeEditor} closeDisabled={saving || confirmationOpen} header={<h2><MenuIcon name="eject"/>{pt ? "Editar armazenamento" : "Edit storage"}</h2>}>
      <div className="storage-editor-content"><p className="storage-editor-device"><strong>{volume.label || volume.partition_name || volume.device}</strong> · {volume.device} · {volume.filesystem || (pt ? "Sem sistema de arquivos" : "No filesystem")}</p>
      <section className="storage-mount-form">
      {(volume.filesystem.startsWith("ntfs") || volume.preparation_kind) ? <NtfsMigrationWizard key={volume.id} volume={volume} onBusy={setSaving} onDone={async mountPath => { setSuccess(pt ? `Partição formatada e montada em ${mountPath}.` : `Partition formatted and mounted at ${mountPath}.`); await refresh(); }}/> : volume.blocked_reason ? <div className="storage-warning" role="status">{volume.blocked_reason}</div> : <>
        <label htmlFor="storage-name">{pt ? "Nome do armazenamento" : "Storage name"}</label>
        <input ref={nameRef} id="storage-name" value={name} maxLength={16} disabled={saving || confirmationOpen} onChange={event => setName(storageNameInput(event.target.value))} autoComplete="off" spellCheck={false} aria-describedby="storage-name-hint"/>
        <p id="storage-name-hint">{pt ? "Use até 16 caracteres e inicie o nome com uma letra ou número. São permitidos: letras (sem acentos), números, ponto, hífen e sublinhado." : "Use up to 16 characters and start the name with a letter or number. Allowed: unaccented letters, numbers, dots, hyphens and underscores."}</p>
        <p>{pt ? "Pasta de montagem:" : "Mount folder:"} <strong className="storage-mount-path">/mnt/{name || "…"}</strong></p>
        <label className="preference-switch"><input type="checkbox" checked={startup} disabled={saving || confirmationOpen} onChange={event => setStartup(event.target.checked)}/><span className="preference-switch-track"/><span>{pt ? "Montar ao iniciar o sistema" : "Mount at system startup"}</span></label>
        <label className="preference-switch"><input type="checkbox" checked={shared} disabled={saving || confirmationOpen || volume.shared_between_systems} onChange={event => setShared(event.target.checked)}/><span className="preference-switch-track"/><span>{pt ? "Usar em mais de um sistema Linux" : "Use in more than one Linux system"}</span></label>
        <p>{volume.shared_between_systems ? (pt ? "Este armazenamento já está liberado para outros sistemas Linux: qualquer conta pode gravar nele." : "This storage is already open to other Linux systems: any account can write to it.") : pt ? "Para outro Linux instalado neste computador também gravar aqui. Ao salvar, todos os arquivos e pastas atuais passam a poder ser alterados por qualquer conta de qualquer sistema, e os novos também. Nada é apagado; em discos grandes isso pode levar alguns minutos." : "So another Linux installed on this computer can write here too. On save, every current file and folder becomes changeable by any account on any system, and new ones too. Nothing is deleted; large drives may take a few minutes."}</p>
        <p>{pt ? "A configuração usa UUID para identificar a partição e nofail para não bloquear a inicialização se o disco estiver desconectado. Salvar também monta o volume agora." : "Uses UUID to identify the partition and nofail so a disconnected drive does not block startup. Saving also mounts the volume now."}{!shared || volume.shared_between_systems ? (pt ? " As permissões dos arquivos permanecem como estão." : " Existing file permissions are preserved.") : ""}</p>
        <button type="button" className="preference-outline" disabled={!valid || saving || confirmationOpen} onClick={() => setConfirm("save")}><MenuIcon name="floppy"/>{saving ? (pt ? "Aplicando…" : "Applying…") : (pt ? "Salvar automontagem" : "Save automatic mounting")}</button>
      </>}
      </section>
      {error && <div className="preferences-error" role="alert">{error}</div>}
      {!volume.filesystem.startsWith("ntfs") && <button type="button" className="preference-outline storage-editor-cancel" disabled={saving || confirmationOpen} onClick={closeEditor}>{pt ? "Fechar" : "Close"}</button>}
      </div></ModalSurface>
    </div>, document.body)}
    {error && !volume && <div className="preferences-error" role="alert">{error}</div>}
    {success && <p role="status">{success}</p>}
    {confirm === "save" && volume && <ConfirmModal title={pt ? "Aplicar automontagem?" : "Apply automatic mounting?"} message={pt ? `Dispositivo: ${volume.device}. O volume será desmontado sem forçar, receberá o nome e label “${name}”, e sua configuração em /etc/fstab será atualizada para /mnt/${name}. Montagem na inicialização: ${startup ? "ativada" : "desativada"}.${shared && !volume.shared_between_systems ? " Todos os arquivos e pastas do volume passarão a poder ser alterados por qualquer conta de qualquer sistema Linux." : ""} A configuração do systemd será recarregada e o volume será montado nesse caminho, sem reiniciar o computador. Feche jogos e arquivos abertos nesse disco. O sistema pedirá a senha de administrador. Não haverá formatação nem exclusão de arquivos. Se uma etapa falhar, a mensagem informará o que já foi concluído.` : `Device: ${volume.device}. The volume will be unmounted without force, renamed to “${name}” (partition and label), configured in /etc/fstab at /mnt/${name}, then systemd will reload its configuration and the volume will be mounted there, without rebooting. Startup mounting: ${startup ? "enabled" : "disabled"}.${shared && !volume.shared_between_systems ? " Every file and folder on the volume becomes changeable by any account on any Linux system." : ""} Close games and files on this drive. The system will ask for the administrator password. No formatting or file deletion. Any failure will report completed steps.`} confirmText={pt ? "Autorizar e salvar" : "Authorize and save"} cancelText={pt ? "Cancelar" : "Cancel"} busy={saving} onConfirm={() => void save()} onCancel={() => { if (!saving) setConfirm(null); }}/>}
    {removeEntry && <ConfirmModal title={pt ? "Remover entrada do fstab?" : "Remove fstab entry?"}
      message={pt ? `${entryName(removeEntry).name} — ${removeEntry.mount_path}\n\n${removeEntry.line}\n\n${removeEntry.automatic ? "Essa linha deixará de montar o disco automaticamente ao iniciar o sistema." : "Essa linha já usa noauto. Sua configuração de montagem manual será removida."} O disco continuará montado agora, caso já esteja em uso. Nenhum arquivo, partição ou pasta será apagado. A alteração remove somente essa linha de /etc/fstab, mantém as outras entradas e recarrega o systemd. O comentário do PeliGames será removido se não restar nenhuma entrada gerenciada. ${!removeEntry.managed ? "Esta entrada não está identificada como gerenciada pelo PeliGames. " : ""}O sistema pedirá autorização de administrador.` : `${entryName(removeEntry).name} — ${removeEntry.mount_path}\n\n${removeEntry.line}\n\n${removeEntry.automatic ? "This line will no longer mount the drive automatically at startup." : "This line already uses noauto. Its manual mount configuration will be removed."} The current mount stays active. No files, partitions or folders are deleted. Only this line is removed from /etc/fstab; other entries are preserved and systemd is reloaded. The PeliGames comment is removed after its last managed entry. ${!removeEntry.managed ? "This entry is not marked as managed by PeliGames. " : ""}Administrator authorization is required.`}
      confirmText={pt ? "Autorizar e remover" : "Authorize and remove"} cancelText={pt ? "Cancelar" : "Cancel"} busy={saving} onConfirm={() => void removeFstab()} onCancel={() => { if (!saving) { setRemoveEntry(null); triggerRef.current?.focus(); } }}/>}
    {operationFailure && <ResultModal title={operationFailure.title} message={operationFailure.message} type="error" logs="" showLogButton={false} onClose={() => setOperationFailure(null)}/>}</>;
}

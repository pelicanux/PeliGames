import { useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../MenuIcon";
import { useI18n } from "../../i18n/I18nContext";
import { storageNameInput } from "./storageName";
import type { Volume } from "./DiskAutomountPreferences";

type Props = { volume: Volume; onBusy: (busy: boolean) => void; onDone: (mountPath: string) => Promise<void> };
export function NtfsMigrationWizard({ volume, onBusy, onDone }: Props) {
  const { language } = useI18n(); const pt = language === "pt";
  const [step, setStep] = useState(0), [backup, setBackup] = useState(false);
  const [name, setName] = useState(storageNameInput(volume.label || "Jogos"));
  const [filesystem, setFilesystem] = useState<"ext4" | "btrfs">("ext4"), [startup, setStartup] = useState(true);
  const [confirmation, setConfirmation] = useState(""), [busy, setBusy] = useState(false), [error, setError] = useState("");
  // A failed destructive request is never retried automatically. Refresh the device list first.
  const [attempted, setAttempted] = useState(false);
  const wizardRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const body = wizardRef.current?.closest<HTMLElement>(".modal-scroll-body");
    if (body) body.scrollTop = 0;
  }, [step]);
  const confirmationWord = pt ? "confirmar" : "confirm";
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
        confirmation_word: confirmation, backup_confirmed: backup,
      } }) : await invoke<{mount_path:string}>("format_ntfs_storage", {id:volume.id, uuid:volume.uuid, name, filesystem, mountAtStartup:startup, confirmationDevice:volume.device, confirmationWord:confirmation, backupConfirmed:backup});
      await onDone(result.mount_path);
    } catch (e) { setError(String(e)); } finally { setBusy(false); onBusy(false); }
  };
  return <div ref={wizardRef} className="storage-ntfs-wizard">
    <p className="storage-wizard-step" role="status">{pt ? "Etapa" : "Step"} {step + 1}/4</p>
    {step === 0 && <>
      <h3>{wholeDisk ? (pt ? "Preparar este disco vazio para jogos" : "Prepare this empty drive for games") : freeSpace ? (pt ? "Criar uma partição no espaço livre" : "Create a partition in unallocated space") : pt ? "Preparar esta partição para jogos no Linux" : "Prepare this partition for Linux games"}</h3>
      {!preparing && <p>{pt ? "O Linux consegue reconhecer NTFS. Porém, diferenças de permissões, links e nomes de arquivos podem causar falhas ao iniciar jogos, bugs e crashes com Proton/Wine." : "Linux can recognize NTFS. However, differences in permissions, links and filenames may cause launch failures, bugs and crashes with Proton/Wine."}</p>}
      {!preparing && <div className="storage-warning"><p>{pt ? "Você pode manter o NTFS e sair deste assistente. Se optar por ext4 ou Btrfs, será necessário formatar a partição, apagando os dados existentes. Não é uma conversão que preserva arquivos." : "You can keep NTFS and leave this wizard. Switching to ext4 or Btrfs requires formatting the partition and erasing existing data. This is not a conversion that preserves files."}</p></div>}
      {preparing && <div className="storage-warning"><p>{wholeDisk ? (pt ? "Será usada uma tabela GPT e criada uma partição ocupando o espaço disponível do disco. Este fluxo só aceita discos sem partições existentes." : "A GPT table and a partition using the available drive space will be created. This flow only accepts drives without existing partitions.") : freeSpace ? (pt ? "Uma nova partição será criada somente neste espaço livre. As partições existentes no disco serão preservadas." : "A new partition will be created only in this free space. Existing partitions will be preserved.") : (pt ? "Somente a partição selecionada será formatada. As outras partições do mesmo HD ou SSD serão preservadas." : "Only the selected partition will be formatted. Other partitions on the same drive will be preserved.")}</p></div>}
      <p>{pt ? "ext4 e Btrfs são sistemas de arquivos Linux. O Windows não os acessa normalmente sem ferramentas adicionais." : "ext4 and Btrfs are Linux filesystems. Windows does not normally access them without additional tools."}</p>
    </>}
    {step === 1 && <>
      <h3>{pt ? "Proteja seus arquivos antes de continuar" : "Protect your files before continuing"}</h3>
      <div className="storage-warning storage-danger-warning" role="alert"><strong>{wholeDisk ? (pt ? "A preparação alterará a tabela do disco e apagará quaisquer dados nele." : "Preparation changes the drive partition table and erases any data on it.") : freeSpace ? (pt ? "Dados antigos ainda presentes neste espaço livre poderão ser apagados." : "Old data remaining in this unallocated space may be erased.") : pt ? "A formatação apagará TODOS os dados desta partição." : "Formatting will erase ALL data on this partition."}</strong><p>{preparing ? (pt ? "Confira o dispositivo e a capacidade. Mesmo sem um sistema de arquivos reconhecido, podem existir dados antigos recuperáveis. Copie o que precisar preservar antes de continuar. Esta operação não pode ser desfeita pela lixeira." : "Check the device and capacity. Even without a recognized filesystem, recoverable old data may remain. Back up anything you need before continuing. This operation cannot be undone through the trash.") : pt ? "Seus jogos, documentos, fotos e demais arquivos serão perdidos. Caso ela contenha o Windows, essa instalação também será perdida. Copie tudo que quiser preservar para outro armazenamento e confira se o backup abre corretamente. A lixeira não permite desfazer essa operação." : "Games, documents, photos and all other files will be lost. If it contains Windows, that installation will also be lost. Copy everything you need to another drive and verify your backup. This operation cannot be undone through the trash."}</p></div>
      <label className="storage-backup-check"><input type="checkbox" checked={backup} onChange={event => setBackup(event.target.checked)}/><span><strong>{pt ? "Confirmar" : "Confirm"}</strong><small>{preparing ? (pt ? "Conferi o dispositivo e confirmei que este espaço está vazio ou que os dados necessários já foram copiados. Autorizo sua preparação e formatação." : "I verified the device and confirmed this space is empty or backed up. I authorize its preparation and formatting.") : pt ? "Verifiquei meu backup em outro armazenamento e entendo que todos os dados desta partição serão apagados." : "I verified my backup on another drive and understand that all data on this partition will be erased."}</small></span></label>
    </>}
    {step === 2 && <div className="storage-mount-form">
      <h3>{pt ? "Escolha o formato e o nome" : "Choose the filesystem and name"}</h3>
      <fieldset className="storage-filesystem-choices"><legend>{pt ? "Sistema de arquivos" : "Filesystem"}</legend>
        <label><input type="radio" name="storage-format" value="ext4" checked={filesystem === "ext4"} onChange={() => setFilesystem("ext4")}/><span><strong>ext4</strong><small>{pt ? "Opção simples e confiável para uma biblioteca de jogos." : "A simple and reliable choice for a game library."}</small></span></label>
        <label><input type="radio" name="storage-format" value="btrfs" checked={filesystem === "btrfs"} onChange={() => setFilesystem("btrfs")}/><span><strong>Btrfs</strong><small>{pt ? "Sistema Linux com recursos como snapshots e compressão, configuráveis à parte." : "Linux filesystem with features such as snapshots and compression, configured separately."}</small></span></label>
      </fieldset>
      <label htmlFor="ntfs-storage-name">{pt ? "Nome do armazenamento" : "Storage name"}</label>
      <input id="ntfs-storage-name" value={name} maxLength={16} onChange={event => setName(storageNameInput(event.target.value))} autoComplete="off" spellCheck={false} aria-describedby="ntfs-name-hint"/>
      <p id="ntfs-name-hint">{pt ? "Use até 16 caracteres e inicie o nome com uma letra ou número. São permitidos: letras (sem acentos), números, ponto, hífen e sublinhado." : "Use up to 16 characters, starting with a letter or number. Allowed: unaccented letters, numbers, dots, hyphens and underscores."}</p>
      <p>{pt ? "Destino:" : "Destination:"} <strong className="storage-mount-path">/mnt/{name || "…"}</strong></p>
      <label className="preference-switch"><input type="checkbox" checked={startup} onChange={event => setStartup(event.target.checked)}/><span className="preference-switch-track"/><span>{pt ? "Montar ao iniciar o sistema" : "Mount at system startup"}</span></label>
      <p>{pt ? "A partição será montada automaticamente ao concluir, mesmo se a montagem na inicialização estiver desativada." : "The partition is mounted when complete, even if startup mounting is disabled."}</p>
    </div>}
    {step === 3 && <>
      <h3>{wholeDisk ? (pt ? "Confira o disco inteiro antes de preparar" : "Review the entire drive before preparing") : freeSpace ? (pt ? "Confira o espaço livre antes de criar a partição" : "Review unallocated space before creating the partition") : pt ? "Confira a partição antes de apagar" : "Review the partition before erasing"}</h3>
      <dl className="storage-format-review"><dt>{pt ? "Dispositivo" : "Device"}</dt><dd>{volume.device}</dd>{preparing && <><dt>{pt ? "Operação" : "Operation"}</dt><dd>{wholeDisk ? (pt ? "GPT e uma partição no disco vazio" : "GPT and one partition on the empty drive") : freeSpace ? (pt ? "Nova partição somente no espaço livre" : "New partition only in unallocated space") : (pt ? "Formatar somente esta partição" : "Format only this partition")}</dd></>}<dt>{pt ? "Capacidade" : "Capacity"}</dt><dd>{(volume.size / 1024 ** 3).toFixed(1)} GiB</dd><dt>{pt ? "Formato" : "Filesystem"}</dt><dd>{preparing ? (wholeDisk ? "GPT + " : freeSpace ? (pt ? "Nova partição · " : "New partition · ") : "") : "NTFS → "}{filesystem}</dd><dt>{pt ? "Nome" : "Name"}</dt><dd>{name}</dd><dt>{pt ? "Destino" : "Destination"}</dt><dd>/mnt/{name}</dd><dt>{pt ? "Montar na inicialização" : "Mount at startup"}</dt><dd>{startup ? (pt ? "Sim" : "Yes") : (pt ? "Não" : "No")}</dd></dl>
      <div className="storage-warning storage-danger-warning"><strong>{wholeDisk ? (pt ? "O disco acima será preparado e formatado." : "The drive above will be prepared and formatted.") : freeSpace ? (pt ? "Somente o espaço livre selecionado será utilizado. As partições existentes serão preservadas." : "Only the selected unallocated space will be used. Existing partitions will be preserved.") : pt ? "Todos os dados da partição acima serão apagados." : "All data on the partition above will be erased."}</strong><p>{pt ? "Feche os jogos e arquivos abertos nesse armazenamento. O sistema pedirá autorização administrativa quando necessário. O launcher não recebe nem guarda sua senha. Depois de iniciar, aguarde até concluir; não desconecte o disco nem desligue o computador." : "Close games and files on this drive. The system requests administrator authorization when needed; the launcher does not receive or store your password. After starting, wait for completion; do not disconnect the drive or shut down."}</p></div>
      <div className="storage-mount-form"><label htmlFor="storage-format-confirm">{pt ? "Digite " : "Type "}<strong className="storage-confirmation-word">“{confirmationWord}”</strong>{wholeDisk ? (pt ? " para autorizar a preparação e formatação deste disco inteiro." : " to authorize preparing and formatting this entire drive.") : freeSpace ? (pt ? " para autorizar a criação e formatação da nova partição neste espaço livre." : " to authorize creating and formatting a new partition in this free space.") : pt ? " para autorizar que esta partição seja apagada." : " to authorize erasing this partition."}</label><input id="storage-format-confirm" value={confirmation} disabled={busy || attempted} onChange={event => setConfirmation(event.target.value)} aria-label={wholeDisk ? (pt ? "Digite confirmar para preparar e formatar o disco inteiro" : "Type confirm to prepare and format the entire drive") : freeSpace ? (pt ? "Digite confirmar para criar a partição no espaço livre" : "Type confirm to create a partition in unallocated space") : pt ? "Digite confirmar para apagar a partição" : "Type confirm to erase the partition"} autoComplete="off" spellCheck={false}/></div>
    </>}
    {blocked && <div className="storage-warning" role="alert">{blocked}</div>}
    {error && <div className="preferences-error" role="alert">{error}<p>{pt ? "Feche este assistente e atualize os discos para conferir o estado antes de tentar qualquer outra alteração." : "Close this wizard and refresh drives to inspect the current state before making any further changes."}</p></div>}
    {busy && <p role="status" className="storage-wizard-progress"><MenuIcon name="repair"/>{pt ? "Aguardando autorização ou preparando a partição…" : "Waiting for authorization or preparing the partition…"}</p>}
    <div className="storage-wizard-actions">
      {step > 0 && <button type="button" className="preference-outline" disabled={busy || attempted} onClick={() => { setStep(step - 1); setConfirmation(""); }}>{pt ? "Voltar" : "Back"}</button>}
      {step < 3 ? <button type="button" className="preference-outline" disabled={Boolean(blocked) || (step === 1 && !backup) || (step === 2 && !valid)} onClick={() => setStep(step + 1)}>{step === 0 ? (pt ? "Sim, preparar para Linux" : "Yes, prepare for Linux") : (pt ? "Avançar" : "Next")}</button> : <button type="button" className="btn btn-danger" disabled={busy || attempted || Boolean(blocked) || !backup || !valid || confirmation !== confirmationWord} onClick={() => void format()}>{preparing ? (pt ? "Preparar e formatar" : "Prepare and format") : pt ? "Formatar e apagar esta partição" : "Format and erase this partition"}</button>}
    </div>
  </div>;
}

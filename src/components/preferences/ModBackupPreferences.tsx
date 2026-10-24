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
  const { language } = useI18n(), pt = language === "pt";
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
    const folder = await open({ directory: true, multiple: false, title: pt ? "Onde salvar o backup de todos os mods?" : "Where should the complete mod backup be saved?" });
    if (typeof folder !== "string") return;
    const report = await invoke<Report>("create_mod_backup", { destination: folder });
    setNotice(`${pt ? "Backup salvo em" : "Backup saved to"}: ${report.path}`);
  });
  const restore = () => run(async () => {
    const folder = await open({ multiple: false, filters: [{ name: "Backup Nexus", extensions: ["zip"] }], title: pt ? "Selecione o ZIP de backup Nexus" : "Select the Nexus backup ZIP" });
    if (typeof folder !== "string") return;
    const report = await invoke<Report>("restore_mod_backup", { source: folder });
    setNotice(pt ? `${report.restored} jogos recuperados; ${report.skipped} já existentes preservados.` : `${report.restored} games restored; ${report.skipped} existing games preserved.`);
    if (report.warnings.length) onError(report.warnings.join("\n"));
    await onRestored();
    window.dispatchEvent(new Event("refreshGames"));
  });
  const recoverDisk = () => run(async () => {
    const folder = await open({ directory: true, multiple: false, title: pt ? "Selecione o disco ou a pasta dos jogos" : "Select the drive or game library folder" });
    if (typeof folder !== "string") return;
    const report = await invoke<Report>("recover_mods_from_disk", { directory: folder });
    setNotice(pt ? `${report.restored} jogos Nexus recuperados; ${report.skipped} já existentes preservados.` : `${report.restored} Nexus games restored; ${report.skipped} existing games preserved.`);
    if (report.warnings.length) onError(report.warnings.join("\n"));
    await onRestored();
    window.dispatchEvent(new Event("refreshGames"));
  });
  const deleteBackups = () => run(async () => {
    try {
      setSettings(await invoke<Settings>("delete_all_mod_backups"));
      setNotice(pt ? "Backups internos excluídos. A recuperação automática foi desativada." : "Internal backups deleted. Automatic recovery has been disabled.");
    } finally {
      setConfirmDelete(false);
      window.dispatchEvent(new Event("modBackupSettingsChanged"));
    }
  });
  return <section className="mod-workspace mod-backup-workspace mod-backup-card">
    <header className="mod-intro"><h3>{pt?"Backup e recuperação":"Backup and recovery"}</h3><p>{pt?"Guarde uma cópia ou recupere uma instalação existente. As contas ficam fora do backup.":"Keep a copy or recover an existing installation. Accounts are excluded from backups."}</p></header>
    <div className="mod-backup-actions">
      <div data-preference-id="backup-create" className="mod-setting-row"><div className="mod-row-copy"><span className="mod-component-icon"><MenuIcon name="floppy"/></span><div><strong>{pt?"Guardar uma cópia":"Keep a copy"}</strong><small>{pt?"Cria um ZIP com pacotes, perfis e registros de todos os jogos com mods.":"Creates a ZIP with packages, profiles and records for all modded games."}</small></div></div><button type="button" className="mod-action-primary" disabled={busy} onClick={backup}>{pt?"Criar backup":"Create backup"}</button></div>
      <div data-preference-id="backup-restore" className="mod-setting-row"><div className="mod-row-copy"><span className="mod-component-icon"><MenuIcon name="download"/></span><div><strong>{pt?"Voltar de um backup":"Restore from a backup"}</strong><small>{pt?"Importa seu ZIP. Jogos já configurados são preservados.":"Imports your ZIP. Already configured games are preserved."}</small></div></div><button type="button" className="mod-action-secondary" disabled={busy} onClick={restore}>{pt?"Restaurar":"Restore"}</button></div>
      <div data-preference-id="backup-discover" className="mod-setting-row"><div className="mod-row-copy"><span className="mod-component-icon"><MenuIcon name="folderSearch"/></span><div><strong>{pt?"Encontrar uma instalação anterior":"Find a previous installation"}</strong><small>{pt?"Procura registros de recuperação do PeliGames na pasta dos jogos.":"Finds PeliGames recovery records in your game folders."}</small></div></div><button type="button" className="mod-action-secondary" disabled={busy} onClick={recoverDisk}>{pt?"Procurar no disco":"Search drive"}</button></div>
      <div data-preference-id="backup-delete-all" className="mod-setting-row"><div className="mod-row-copy"><span className="mod-component-icon"><MenuIcon name="trash"/></span><div><strong>{pt?"Apagar todos os backups de mods":"Delete all mod backups"}</strong><small>{pt?"Exclui as cópias internas de recuperação, inclusive as preservadas após desinstalar jogos. Mantém os mods instalados e os ZIPs exportados.":"Deletes internal recovery copies, including copies preserved after uninstalling games. Keeps installed mods and exported ZIPs."}</small></div></div><button type="button" className="mod-action-secondary" disabled={busy || !settings} onClick={()=>setConfirmDelete(true)}>{pt?"Apagar backups":"Delete backups"}</button></div>
    </div>
    <div className="mod-backup-policy"><h4>{pt?"Recuperação por jogo":"Per-game recovery"}</h4>
    <label data-preference-id="backup-per-game" className="preference-switch" style={{ marginTop: 20 }}>
      <input type="checkbox" disabled={busy || !settings} checked={settings?.per_game ?? false} onChange={event => { const perGame = event.target.checked; void run(async () => { setSettings(await invoke<Settings>("set_mod_backup_settings", { perGame })); window.dispatchEvent(new Event("modBackupSettingsChanged")); }); }}/>
      <span className="preference-switch-track"/><span>{pt ? "Cópia de recuperação na pasta de cada jogo" : "Recovery copy in each game folder"}<small>{pt ? "Mantém uma pasta oculta .peligames somente nos jogos com mods Nexus instalados. Ocupa espaço adicional. Ao escanear ou associar o jogo novamente, recupera os mods na .config. As escolhas individuais prevalecem sobre esta opção global." : "Keeps a hidden .peligames folder only for games with installed Nexus mods. Uses extra disk space. Scanning or associating the game again restores mods to .config. Individual choices override this global default."}</small></span>
    </label>
    </div>
    {copies.length > 0 && <ul className="mod-backup-copies" aria-label={pt ? "Cópias de recuperação por jogo" : "Recovery copies by game"}>
      {copies.map(game => <li key={game.game_id} className={game.error ? "is-error" : game.updated && !game.enabled ? "is-stale" : ""}>
        <strong>{game.name}</strong>
        <small>{game.error ? (pt ? `falha: ${game.error}` : `failed: ${game.error}`)
          : game.updating ? (pt ? "atualizando…" : "updating…")
          : game.enabled ? (game.updated ? (pt ? `atualizada em ${backupDate(game.updated, pt)}` : `updated ${backupDate(game.updated, pt)}`) : (pt ? "aguardando a primeira cópia" : "waiting for the first copy"))
          : game.updated ? (pt ? `cópia antiga de ${backupDate(game.updated, pt)}, desligada` : `old copy from ${backupDate(game.updated, pt)}, disabled`) : (pt ? "sem cópia" : "no copy")}
          {game.choice !== null && (pt ? " · escolha do jogo" : " · game's own choice")}</small>
      </li>)}
    </ul>}
    {settings?.synchronizing && <p role="status">{pt ? "Atualizando as cópias de recuperação no disco…" : "Updating recovery copies on disk…"}</p>}
    {busy && <p role="status">{pt ? "Processando os arquivos…" : "Processing files…"}</p>}
    {notice && <p role="status" style={{ overflowWrap: "anywhere" }}>{notice}</p>}
    {settings?.last_error && <p role="alert" className="preferences-error" style={{ whiteSpace: "pre-wrap" }}>{settings.last_error}</p>}
    {confirmDelete && <ConfirmModal title={pt?"Apagar todos os backups de mods?":"Delete all mod backups?"} message={pt?"Esta ação exclui permanentemente as cópias internas de recuperação de todos os jogos e desativa sua criação automática, inclusive as escolhas por jogo. Os mods instalados e os ZIPs exportados serão preservados. Discos desconectados não podem ser limpos; conecte-os antes de continuar.":"This permanently deletes internal recovery copies for all games and disables automatic copies, including per-game choices. Installed mods and exported ZIPs are preserved. Disconnected drives cannot be cleaned; connect them before continuing."} confirmText={pt?"Apagar backups":"Delete backups"} cancelText={pt?"Cancelar":"Cancel"} isDanger busy={busy} onCancel={()=>{if(!busy)setConfirmDelete(false);}} onConfirm={()=>void deleteBackups()}/>}
  </section>;
}

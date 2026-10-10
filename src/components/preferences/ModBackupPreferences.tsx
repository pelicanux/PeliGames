import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { MenuIcon } from "../MenuIcon";
import { useI18n } from "../../i18n/I18nContext";

type Settings = { per_game: boolean; last_error: string; synchronizing: boolean };
type Report = { restored: number; skipped: number; warnings: string[]; path: string };
export function ModBackupPreferences({ onError, onBusy, onRestored }: { onError: (error: string) => void; onBusy: (busy: boolean) => void; onRestored: () => Promise<void> }) {
  const { language } = useI18n(), pt = language === "pt";
  const [settings, setSettings] = useState<Settings | null>(null), [busy, setBusy] = useState(false), [notice, setNotice] = useState("");
  const lock = useRef(false);
  useEffect(() => {
    let mounted = true;
    const refresh = () => void invoke<Settings>("load_mod_backup_settings").then(s => { if (mounted) setSettings(s); }).catch(e => { if (mounted) onError(String(e)); });
    refresh(); const timer = window.setInterval(refresh, 4000);
    return () => { mounted = false; window.clearInterval(timer); };
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
  return <section className="preference-section">
    <h3>{pt ? "Backup e recuperação de mods Nexus" : "Nexus mod backup and recovery"}</h3>
    <p>{pt ? "Crie um ZIP com pacotes, perfis e registros dos jogos com mods Nexus instalados. Na restauração, jogos já configurados são preservados. As contas não entram no backup." : "Create a ZIP with packages, profiles and records for games with installed Nexus mods. Restoring preserves games already configured. Accounts are excluded."}</p>
    <div className="preference-tool-buttons">
      <button type="button" disabled={busy} onClick={backup}><MenuIcon name="floppy"/>{pt ? "Criar backup Nexus" : "Create Nexus backup"}</button>
      <button type="button" disabled={busy} onClick={recoverDisk}><MenuIcon name="folderSearch"/>{pt ? "Encontrar mods no SSD" : "Find mods on drive"}</button>
      <button type="button" disabled={busy} onClick={restore}><MenuIcon name="download"/>{pt ? "Restaurar backup" : "Restore backup"}</button>
    </div>
    <label className="preference-switch" style={{ marginTop: 20 }}>
      <input type="checkbox" disabled={busy || !settings} checked={settings?.per_game ?? false} onChange={event => { const perGame = event.target.checked; void run(async () => { setSettings(await invoke<Settings>("set_mod_backup_settings", { perGame })); window.dispatchEvent(new Event("modBackupSettingsChanged")); }); }}/>
      <span className="preference-switch-track"/><span>{pt ? "Cópia de recuperação na pasta de cada jogo" : "Recovery copy in each game folder"}<small>{pt ? "Mantém uma pasta oculta .peligames somente nos jogos com mods Nexus instalados. Ocupa espaço adicional. Ao escanear ou associar o jogo novamente, recupera os mods na .config. As escolhas individuais prevalecem sobre esta opção global." : "Keeps a hidden .peligames folder only for games with installed Nexus mods. Uses extra disk space. Scanning or associating the game again restores mods to .config. Individual choices override this global default."}</small></span>
    </label>
    {settings?.synchronizing && <p role="status">{pt ? "Atualizando as cópias de recuperação no disco…" : "Updating recovery copies on disk…"}</p>}
    {busy && <p role="status">{pt ? "Processando os arquivos…" : "Processing files…"}</p>}
    {notice && <p role="status" style={{ overflowWrap: "anywhere" }}>{notice}</p>}
    {settings?.last_error && <p role="alert" className="preferences-error" style={{ whiteSpace: "pre-wrap" }}>{settings.last_error}</p>}
  </section>;
}

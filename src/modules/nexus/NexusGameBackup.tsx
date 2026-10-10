import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { HoverTooltip } from "../../components/HoverTooltip";
import { NexusPopup } from "./NexusPopup";
import { useI18n } from "../../i18n/I18nContext";

type BackupSettings = { per_game: boolean; game_overrides?: Record<string, boolean> };
export function NexusGameBackup({ gameId, disabled, onError, onSaved }: { gameId: string; disabled: boolean; onError: (error: string) => void; onSaved: () => void }) {
  const { language } = useI18n(), pt = language === "pt";
  const [settings, setSettings] = useState<BackupSettings | null>(null), [saving, setSaving] = useState(false), [confirmDelete, setConfirmDelete] = useState(false), [deleteError, setDeleteError] = useState("");
  const pending = useRef(false), currentGame = useRef(gameId); currentGame.current = gameId;
  useEffect(() => {
    let active = true; setSettings(null); setConfirmDelete(false); setDeleteError("");
    const refresh = () => { if (pending.current) return; void invoke<BackupSettings>("load_mod_backup_settings").then(value => { if (active && !pending.current) setSettings(value); }).catch(e => { if (active) onError(String(e)); }); };
    refresh(); window.addEventListener("modBackupSettingsChanged", refresh);
    return () => { active = false; window.removeEventListener("modBackupSettingsChanged", refresh); };
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
  return <div className="installation-field nexus-game-backup-control">
    <label><MenuIcon name="floppy" />{pt ? "Backup individual" : "Individual backup"}</label>
    <div className={`neural-startup-switch nexus-game-backup-box${enabled ? " enabled" : ""}`}>
      <HoverTooltip text={pt ? "Salva os pacotes e registros na pasta .peligames deste jogo. Esta escolha vale somente para este jogo." : "Saves packages and records in this game's .peligames folder. This choice applies only to this game."} anchorClassName="nexus-game-backup-tooltip">
        <button type="button" className="nexus-game-backup-toggle" role="switch" aria-label={pt ? "Cópia de recuperação deste jogo" : "Recovery copy for this game"} aria-checked={enabled} disabled={locked} onClick={() => void toggle(!enabled)}>
          <span className="neural-switch-track" aria-hidden="true"><span /></span>
          <span>{pt ? enabled ? "Ativado" : "Desativado" : enabled ? "Enabled" : "Disabled"}</span>
        </button>
      </HoverTooltip>
      <HoverTooltip text={pt ? "Apagar backup deste jogo" : "Delete this game's backup"}>
        <button type="button" className="nexus-game-backup-delete" aria-label={pt ? "Apagar backup deste jogo" : "Delete this game's backup"} disabled={locked} onClick={() => { setDeleteError(""); setConfirmDelete(true); }}><MenuIcon name="trash" /></button>
      </HoverTooltip>
    </div>
    {confirmDelete && <NexusPopup className="nexus-remove-popup" title={pt ? "Apagar backup individual" : "Delete individual backup"} onClose={() => { if (!pending.current) setConfirmDelete(false); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={saving} onClick={() => setConfirmDelete(false)}>{pt ? "Cancelar" : "Cancel"}</button>
      <button type="button" className="btn btn-primary" disabled={locked} onClick={() => void removeBackup()}><MenuIcon name="trash" />{saving ? pt ? "Apagando…" : "Deleting…" : pt ? "Apagar backup" : "Delete backup"}</button>
    </>}>
      <p className="nexus-hint">{pt ? "Apagar a cópia de recuperação dos mods na pasta deste jogo? Esta ação também desativa o backup individual para evitar que a cópia seja recriada automaticamente." : "Delete the mod recovery copy in this game's folder? This also disables individual backup to prevent the copy from being recreated automatically."}</p>
      <p className="nexus-hint">{pt ? "Os mods instalados e os dados na .config serão mantidos. Esta exclusão não pode ser desfeita." : "Installed mods and data in .config will be kept. This deletion cannot be undone."}</p>
      {deleteError && <p role="alert" className="nexus-error">{deleteError}</p>}
    </NexusPopup>}
  </div>;
}

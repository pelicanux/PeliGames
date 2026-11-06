import { useCallback, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nContext";
import { BootSplash, SplashDone } from "../../components/BootSplash";
import { ModalSurface } from "../../components/ModalSurface";
import { notifyError } from "../../services/notifications";
import "./resetRecovery.css";

type Status = { state: "none" | "checking" | "cleanup" | "recovery"; error?: string | null; backups: string[] };
function ResetHealth({ onFailure, onSuccess }: { onFailure: (error: unknown) => void; onSuccess: () => void }) {
  useEffect(() => {
    let active = true;
    // This component mounts with the loaded launcher, never with its lazy fallback
    // or ErrorBoundary. Give the new UI a responsive startup period before commit.
    const timer = window.setTimeout(() => {
      if (!active) return;
      void (async () => {
        const probe = "peligames.reset.health";
        localStorage.setItem(probe, "ready");
        if (localStorage.getItem(probe) !== "ready") throw new Error("Não foi possível validar o armazenamento da nova interface.");
        localStorage.removeItem(probe);
        if (!("__PELI_UI_PREVIEW__" in window)) await invoke("launcher_reset_confirm_health");
      })().then(() => { if (active) onSuccess(); }).catch(error => { if (active) onFailure(error); });
    }, 10000);
    return () => { active = false; window.clearTimeout(timer); };
  }, [onFailure, onSuccess]);
  return null;
}

export function ResetStartupGate({ children }: { children: (health: ReactNode) => ReactNode }) {
  const {t: tx} = useI18n();

  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState("");
  useEffect(() => {
    let active = true;
    void invoke<Status>("launcher_reset_status").then(value => { if (active) setStatus(value); }).catch(error => {
      if (active) { setFailure(String(error)); setStatus({ state: "recovery", backups: [] }); }
    });
    return () => { active = false; };
  }, []);
  // Stable callbacks keep the health grace period independent of parent renders.
  const [callbacks] = useState(() => ({
    success: () => setStatus({ state: "none", backups: [] }),
    failure: (error: unknown) => { setFailure(String(error)); setStatus(previous => ({ ...previous!, state: "recovery" })); },
  }));
  const healthFailure = useCallback(async (error: unknown) => {
    const latest = "__PELI_UI_PREVIEW__" in window ? status : await invoke<Status>("launcher_reset_status").catch(() => null);
    if (latest?.state === "cleanup") {
      notifyError(tx("ui-reset-startup-gate", "reset_cleanup"), error);
      callbacks.success(); // Already committed; retry cleanup next start, never offer partial rollback.
    } else callbacks.failure(error);
  }, [status?.state, tx, callbacks]);
  const recover = async (restore: boolean) => {
    if (busy) return;
    setBusy(true); setFailure("");
    try {
      if ("__PELI_UI_PREVIEW__" in window) {
        if (restore) throw new Error(tx("ui-reset-startup-gate", "preview_no_files_will_change_and_no_program_will_restart"));
      } else await invoke(restore ? "launcher_reset_restore" : "launcher_reset_continue");
      if (!restore) setStatus(previous => ({ ...previous!, state: "checking" }));
    } catch (error) { setFailure(String(error)); }
    finally { setBusy(false); }
  };
  if (!status) return <BootSplash />;
  if (status.state === "recovery") return <div className="reset-recovery-screen"><BootSplash /><SplashDone /><div className="modal-overlay">
    <ModalSurface className="modal-content dialog-glass steam-manual-popup maintenance-popup reset-recovery" hideClose role="dialog" aria-modal="true" aria-labelledby="reset-recovery-title"
      header={<h2 id="reset-recovery-title">{tx("ui-reset-startup-gate", "recover_reset")}</h2>}
      footer={<div className="nexus-popup-footer">
        <button className="btn btn-secondary" disabled={busy} onClick={() => void recover(false)}>{tx("ui-reset-startup-gate", "try_new_configuration")}</button>
        <button className="btn btn-primary" disabled={busy} onClick={() => void recover(true)}>{busy ? (tx("ui-reset-startup-gate", "please_wait")) : (tx("ui-reset-startup-gate", "restore_and_restart"))}</button>
      </div>}>
      <p>{tx("ui-reset-startup-gate", "a_previous_maintenance_operation_was_interrupted_previous_files_were_preserved_you_can_r")}</p>
      <p>{tx("ui-reset-startup-gate", "restoring_restarts_the_app_and_preserves_the_new_configuration_until_recovery_is_validat")}</p>
      {(failure || status.error) && <p role="alert">{failure || status.error}</p>}
      {status.backups.length > 0 && <ul className="maintenance-paths">{status.backups.map(path => <li key={path}><code>{path}</code></li>)}</ul>}
    </ModalSurface></div></div>;
  return children(status.state !== "none" ? <ResetHealth onSuccess={callbacks.success} onFailure={healthFailure} /> : null);
}

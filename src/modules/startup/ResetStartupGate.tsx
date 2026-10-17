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
  const pt = useI18n().language === "pt";
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
      notifyError(pt ? "Limpeza da redefinição" : "Reset cleanup", error);
      callbacks.success(); // Already committed; retry cleanup next start, never offer partial rollback.
    } else callbacks.failure(error);
  }, [status?.state, pt, callbacks]);
  const recover = async (restore: boolean) => {
    if (busy) return;
    setBusy(true); setFailure("");
    try {
      if ("__PELI_UI_PREVIEW__" in window) {
        if (restore) throw new Error(pt ? "Prévia: nenhum arquivo será alterado ou programa reiniciado." : "Preview: no files will change and no program will restart.");
      } else await invoke(restore ? "launcher_reset_restore" : "launcher_reset_continue");
      if (!restore) setStatus(previous => ({ ...previous!, state: "checking" }));
    } catch (error) { setFailure(String(error)); }
    finally { setBusy(false); }
  };
  if (!status) return <BootSplash />;
  if (status.state === "recovery") return <><BootSplash /><SplashDone /><div className="modal-overlay">
    <ModalSurface className="modal-content dialog-glass steam-manual-popup maintenance-popup reset-recovery" hideClose role="dialog" aria-modal="true" aria-labelledby="reset-recovery-title"
      header={<h2 id="reset-recovery-title">{pt ? "Recuperar redefinição" : "Recover reset"}</h2>}
      footer={<div className="nexus-popup-footer">
        <button className="btn btn-secondary" disabled={busy} onClick={() => void recover(false)}>{pt ? "Tentar configuração nova" : "Try new configuration"}</button>
        <button className="btn btn-primary" disabled={busy} onClick={() => void recover(true)}>{busy ? (pt ? "Aguarde…" : "Please wait…") : (pt ? "Restaurar e reiniciar" : "Restore and restart")}</button>
      </div>}>
      <p>{pt ? "A redefinição anterior não teve a inicialização validada. As cópias antigas foram preservadas. Você pode tentar novamente ou restaurar os arquivos anteriores." : "The previous reset did not pass startup validation. Old copies were preserved. You can retry or restore the previous files."}</p>
      <p>{pt ? "A restauração reinicia o programa e preserva a configuração nova até validar a recuperação. Contas desconectadas precisam ser vinculadas novamente." : "Restoring restarts the app and preserves the new configuration until recovery is validated. Disconnected accounts must be linked again."}</p>
      {(failure || status.error) && <p role="alert">{failure || status.error}</p>}
      {status.backups.length > 0 && <ul className="maintenance-paths">{status.backups.map(path => <li key={path}><code>{path}</code></li>)}</ul>}
    </ModalSurface></div></>;
  return children(status.state !== "none" ? <ResetHealth onSuccess={callbacks.success} onFailure={healthFailure} /> : null);
}

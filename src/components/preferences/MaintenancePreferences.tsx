import { useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nContext";
import { MenuIcon } from "../MenuIcon";
import { NexusPopup } from "../../modules/nexus/NexusPopup";
import { notifyError } from "../../services/notifications";

type Plan = { action: "reset" | "uninstall"; paths: string[] };
export function MaintenancePreferences({ uninstall = false }: { uninstall?: boolean }) {
  const pt = useI18n().language === "pt";
  const [plan, setPlan] = useState<Plan | null>(null);
  const [busy, setBusy] = useState(false);
  const [accepted, setAccepted] = useState(false);
  const [restartRequired, setRestartRequired] = useState(false);
  const lock = useRef(false);
  useLayoutEffect(() => {
    if (!restartRequired) return;
    const blockOutside = (event: Event) => {
      if (event.target instanceof Element && event.target.closest(".maintenance-restart-required")) return;
      event.preventDefault(); event.stopImmediatePropagation();
    };
    // Notifications normally remain usable above dialogs. A reset is different:
    // navigation must not unmount the mandatory restart screen.
    window.addEventListener("pointerdown", blockOutside, true);
    window.addEventListener("click", blockOutside, true);
    return () => { window.removeEventListener("pointerdown", blockOutside, true); window.removeEventListener("click", blockOutside, true); };
  }, [restartRequired]);
  const title = uninstall ? (pt ? "Desinstalar AppImage" : "Uninstall AppImage") : (pt ? "Redefinir PeliGames" : "Reset PeliGames");
  const description = uninstall
    ? (pt ? "Remove esta instalação AppImage e seus atalhos. Os dados e conexões são preservados; use Redefinir PeliGames para apagá-los." : "Removes this AppImage installation and its shortcuts. Data and connections are preserved; use Reset PeliGames to erase them.")
    : (pt ? "Remove contas vinculadas, preferências, histórico, capas, caches, downloads de mods e prefixos guardados nas pastas do PeliGames. Mantém o programa instalado e preserva jogos em pastas externas." : "Removes linked accounts, preferences, history, covers, caches, mod downloads and prefixes stored in PeliGames folders. Keeps the program installed and games in external folders.");
  const review = async () => {
    if (lock.current) return;
    lock.current = true; setBusy(true);
    try { setPlan(await invoke<Plan>("launcher_maintenance_plan", { action: uninstall ? "uninstall" : "reset" })); setAccepted(false); }
    catch (error) { notifyError(title, String(error)); }
    finally { lock.current = false; setBusy(false); }
  };
  const apply = async () => {
    if (!plan || !accepted || lock.current) return;
    lock.current = true; setBusy(true);
    try {
      // The visual preview must never remove real files or credentials.
      if ("__PELI_UI_PREVIEW__" in window) {
        if (plan.action === "reset") setRestartRequired(true);
        else throw new Error(pt ? "Demonstração: nenhuma conta ou arquivo foi removido." : "Preview: no accounts or files were removed.");
      } else {
        await invoke("launcher_maintenance_apply", { action: plan.action, confirmedPaths: plan.paths });
        if (plan.action === "reset") setRestartRequired(true);
      }
    } catch (error) { notifyError(title, String(error)); }
    finally { lock.current = false; setBusy(false); }
  };
  const restart = async () => {
    if (lock.current) return;
    lock.current = true; setBusy(true);
    try {
      if ("__PELI_UI_PREVIEW__" in window) throw new Error(pt ? "Prévia: nenhum arquivo será apagado e nenhum programa será reiniciado." : "Preview: no files will be deleted and no program will restart.");
      await invoke("launcher_maintenance_restart");
    } catch (error) { notifyError(title, String(error)); }
    finally { lock.current = false; setBusy(false); }
  };
  if (restartRequired) return <NexusPopup className="steam-manual-popup maintenance-popup maintenance-restart-required" title={pt ? "Reinício necessário" : "Restart required"} titleIcon={<MenuIcon name="repair"/>} onClose={() => {}} footer={
    <button autoFocus type="button" className="btn btn-primary" disabled={busy} aria-busy={busy} onClick={() => void restart()}><MenuIcon name="repair"/>{busy ? (pt ? "Reiniciando…" : "Restarting…") : (pt ? "Reiniciar PeliGames" : "Restart PeliGames")}</button>
  }><p>{pt ? "A redefinição está pronta. Ao reiniciar, as pastas antigas serão renomeadas e preservadas até validar a nova configuração. Só então a cópia será apagada. O programa ficará bloqueado até o reinício." : "The reset is ready. Restarting renames and preserves old folders until the new configuration is validated. Only then is the copy deleted. The program stays locked until it restarts."}</p></NexusPopup>;
  return <section className="preference-section" data-preference-id={uninstall ? "appimage-uninstall" : "launcher-reset"}>
    <h3>{title}</h3><p>{description}</p>
    <button type="button" className="preference-outline danger" disabled={busy} onClick={() => void review()}><MenuIcon name={uninstall ? "trash" : "repair"}/>{title}</button>
    {plan && <NexusPopup className="steam-manual-popup maintenance-popup" title={title} titleIcon={<MenuIcon name="repair"/>} onClose={() => { if (!busy) setPlan(null); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => setPlan(null)}>{pt ? "Cancelar" : "Cancel"}</button>
      <button type="button" className="btn btn-danger" disabled={busy || !accepted} aria-busy={busy} onClick={() => void apply()}>{busy ? (pt ? "Aguarde…" : "Please wait…") : title}</button>
    </>}>
      <p>{description}</p>
      <p>{pt ? "O programa será fechado para concluir a operação. Feche outras instâncias, jogos, instalações e downloads antes de continuar." : "The program will close to complete the operation. Close other instances, games, installations and downloads first."}</p>
      <strong>{pt ? "Caminhos da operação:" : "Operation paths:"}</strong>
      <ul className="maintenance-paths">{plan.paths.map(path => <li key={path}><code>{path}</code></li>)}</ul>
      {!uninstall && <p>{pt ? "As pastas serão renomeadas no reinício. Após validar a nova inicialização, a cópia será apagada, inclusive arquivos adicionados manualmente. Se houver interrupção, será oferecida a recuperação. As contas nos serviços não são excluídas; credenciais removidas do chaveiro precisam ser vinculadas novamente." : "Folders are renamed on restart. Once the new startup is validated, the copy is deleted, including manually added files. Recovery is offered after an interruption. Service accounts are not deleted; removed keyring credentials must be linked again."}</p>}
      <label className="maintenance-confirm"><input type="checkbox" checked={accepted} disabled={busy} onChange={event => setAccepted(event.target.checked)}/><span>{pt ? "Conferi os caminhos e entendo que esta remoção é permanente." : "I reviewed the paths and understand this removal is permanent."}</span></label>
    </NexusPopup>}
  </section>;
}

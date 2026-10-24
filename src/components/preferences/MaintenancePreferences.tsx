import { useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nContext";
import { MenuIcon } from "../MenuIcon";
import { NexusPopup } from "../../modules/nexus/NexusPopup";
import { notifyError } from "../../services/notifications";

type ModGame = { id: string; name: string; directory: string; device: string; available: boolean; portable: boolean; mods: number };
type Plan = { action: "reset" | "uninstall"; paths: string[]; mod_games?: ModGame[]; installed_games?: {name: string; directory: string; available: boolean}[]; mod_revision?: string; pelirun?: string[] };
export function MaintenancePreferences({ uninstall = false }: { uninstall?: boolean }) {
  const pt = useI18n().language === "pt";
  const [plan, setPlan] = useState<Plan | null>(null);
  const [busy, setBusy] = useState(false);
  const [accepted, setAccepted] = useState(false);
  const [removeMods, setRemoveMods] = useState(false);
  const [preserveAccounts, setPreserveAccounts] = useState(false);
  const [preserveGames, setPreserveGames] = useState(false);
  const [removePelirun, setRemovePelirun] = useState(false);
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
    : (pt ? "Remove preferências, histórico, capas, caches, prefixos guardados nas pastas do PeliGames. Permite preservar as contas conectadas e os registros dos jogos do Pelinstall. Preserva os jogos externos e o gerenciamento dos mods Nexus junto de cada jogo, salvo se você escolher removê-los." : "Removes preferences, history, covers, caches, prefixes stored in PeliGames folders. Lets you preserve connected accounts and Pelinstall game registrations. Keeps external games and Nexus mod management beside each game unless you choose to remove the mods.");
  const review = async () => {
    if (lock.current) return;
    lock.current = true; setBusy(true);
    try { setPlan(await invoke<Plan>("launcher_maintenance_plan", { action: uninstall ? "uninstall" : "reset" })); setAccepted(false); setRemoveMods(false); setPreserveAccounts(false); setPreserveGames(false); setRemovePelirun(false); }
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
        await invoke("launcher_maintenance_apply", { action: plan.action, confirmedPaths: plan.paths, removeMods, preserveAccounts, preserveGames, confirmedModRevision: plan.mod_revision, removePelirun });
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
      <button type="button" className="btn btn-danger" disabled={busy || !accepted || Boolean(plan.mod_games?.some(game => !game.available && (removeMods || !game.portable)))} aria-busy={busy} onClick={() => void apply()}>{busy ? (pt ? "Aguarde…" : "Please wait…") : title}</button>
    </>}>
      <p>{description}</p>
      <p>{pt ? "O programa será fechado para concluir a operação. Feche outras instâncias, jogos, instalações e downloads antes de continuar." : "The program will close to complete the operation. Close other instances, games, installations and downloads first."}</p>
      <strong>{pt ? "Caminhos da operação:" : "Operation paths:"}</strong>
      <ul className="maintenance-paths">{plan.paths.map(path => <li key={path}><code>{path}</code></li>)}</ul>
      {!uninstall && <p>{pt ? "As pastas serão renomeadas no reinício. Após validar a nova inicialização, a cópia será apagada, inclusive arquivos adicionados manualmente. Se houver interrupção, será oferecida a recuperação. As contas nos serviços não são excluídas. Se você não escolher preservá-las, será necessário conectá-las novamente." : "Folders are renamed on restart. Once the new startup is validated, the copy is deleted, including manually added files. Recovery is offered after an interruption. Service accounts are not deleted. Unless you choose to preserve connections, you will need to sign in again."}</p>}
      {!uninstall && <section className="maintenance-games-review">
        <label className="maintenance-confirm"><input type="checkbox" checked={preserveGames} disabled={busy} onChange={event => { setPreserveGames(event.target.checked); setAccepted(false); }}/><span>{pt ? "Preservar jogos instalados ou adicionados pelo Pelinstall" : "Preserve games installed or added through Pelinstall"}</span></label>
        <p>{pt ? "Mantém os registros para recuperar os jogos no mesmo caminho após redefinir. Jogos em discos desconectados continuam registrados e reaparecem quando o disco estiver montado e os arquivos estiverem disponíveis. Runners armazenados nas pastas de redefinição precisarão ser instalados novamente." : "Keeps registrations to recover games at the same paths after resetting. Games on disconnected disks stay registered and return when mounted and their files are available. Runners stored in reset folders will need reinstalling."}</p>
        {preserveGames && Boolean(plan.installed_games?.length) && <ul className="maintenance-paths maintenance-game-list" tabIndex={0} aria-label={pt ? "Jogos do Pelinstall" : "Pelinstall games"}>{plan.installed_games!.map(game => <li key={game.directory}><strong>{game.name}</strong><br/><code>{game.directory}</code>{!game.available && <p>{pt ? "Instalação indisponível. Verifique o disco, a montagem e os arquivos. O registro será preservado." : "Installation unavailable. Check the disk, mount and files. Its registration will be preserved."}</p>}</li>)}</ul>}
      </section>}
      {!uninstall && <section className="maintenance-accounts-review">
        <label className="maintenance-confirm"><input type="checkbox" checked={preserveAccounts} disabled={busy} onChange={event => { setPreserveAccounts(event.target.checked); setAccepted(false); }}/><span>{pt ? "Preservar contas conectadas" : "Preserve connected accounts"}</span></label>
        <p>{preserveAccounts
          ? (pt ? "Mantém as conexões do Matrix, Discord e Nexus Mods, as credenciais no chaveiro e os dados de sessão necessários após o reinício. As demais preferências serão redefinidas." : "Keeps Matrix, Discord and Nexus Mods connections, keyring credentials and session data needed after restarting. Other preferences will be reset.")
          : (pt ? "Desconecta Matrix, Discord e Nexus Mods e remove suas credenciais locais. Será necessário entrar novamente." : "Disconnects Matrix, Discord and Nexus Mods and removes their local credentials. You will need to sign in again.")}</p>
      </section>}
      {!uninstall && <section className="maintenance-mods-review">
        <label className="maintenance-confirm"><input type="checkbox" checked={removeMods} disabled={busy} onChange={event => { setRemoveMods(event.target.checked); setAccepted(false); }}/><span>{pt ? "Remover também todos os mods gerenciados pelo Nexus" : "Also remove all Nexus-managed mods"}</span></label>
        <p>{removeMods
          ? (pt ? "Remove os mods de todos os jogos listados e restaura os originais preservados. Os jogos e saves permanecem. Arquivos antigos sem registro não são apagados automaticamente." : "Removes mods from every listed game and restores preserved originals. Games and saves remain. Untracked older files are not deleted automatically.")
          : (pt ? "Os registros, pacotes e originais preservados serão mantidos na pasta de cada jogo. Após redefinir, o PeliGames recupera o gerenciamento automaticamente. O índice de localização dos mods é preservado." : "Records, packages and preserved originals stay beside each game. After resetting, PeliGames automatically recovers mod management. The mod location index is preserved.")}</p>
        {Boolean(plan.mod_games?.length) && <ul className="maintenance-paths maintenance-game-list" tabIndex={0} aria-label={pt ? "Jogos com mods gerenciados" : "Games with managed mods"}>{plan.mod_games!.map(game => <li key={game.id}><strong>{game.name}</strong> · {game.mods} mods<br/><code>{game.directory}</code>{!game.available && <p role="alert">{pt ? `Não foi possível acessar este jogo. Dispositivo registrado: ${game.device}. Verifique se o dispositivo está conectado e montado no caminho original antes de remover os mods ou migrar seus registros.` : `This game cannot be accessed. Recorded device: ${game.device}. Check that it is connected and mounted at the original path before removing mods or migrating records.`}</p>}</li>)}</ul>}
      </section>}
      {!uninstall && <section className="maintenance-pelirun-review">
        <label className="maintenance-confirm"><input type="checkbox" checked={removePelirun} disabled={busy || !plan.pelirun?.length} onChange={event => { setRemovePelirun(event.target.checked); setAccepted(false); }}/><span>{pt ? "Remover também o PeliRun" : "Also remove PeliRun"}</span></label>
        <p>{pt ? "O PeliRun é o pequeno programa do PeliGames que aplica as configurações de cada jogo (MangoHUD, Proton, opções de inicialização, mods) quando o jogo é aberto pela Steam ou por outro launcher. Ele fica fora das pastas do PeliGames para que esses jogos continuem abrindo mesmo depois de redefinir." : "PeliRun is PeliGames' small program that applies each game's settings (MangoHUD, Proton, launch options, mods) when the game is opened from Steam or another launcher. It lives outside PeliGames' folders so those games keep opening after a reset."}</p>
        <p>{!plan.pelirun?.length
          ? (pt ? "O PeliRun não está instalado neste computador." : "PeliRun is not installed on this computer.")
          : removePelirun
            ? (pt ? "Feche a Steam antes de continuar. O PeliGames tira a chamada do PeliRun das opções de inicialização dos jogos na Steam, mantendo o restante da linha, e apaga as pastas abaixo. Linhas copiadas à mão para outros launchers (Heroic, Lutris) precisam ser apagadas por você." : "Close Steam before continuing. PeliGames removes the PeliRun call from the games' Steam launch options, keeping the rest of the line, and deletes the folders below. Lines you copied by hand into other launchers (Heroic, Lutris) must be removed by you.")
            : (pt ? "Sem esta opção, só os perfis de configuração do PeliRun são apagados; o programa continua, e os jogos da Steam abrem sem os ajustes do PeliGames." : "Without this option, only PeliRun's settings profiles are deleted; the program stays, and Steam games open without PeliGames' adjustments.")}</p>
        {removePelirun && Boolean(plan.pelirun?.length) && <ul className="maintenance-paths" aria-label={pt ? "Pastas do PeliRun" : "PeliRun folders"}>{plan.pelirun!.map(path => <li key={path}><code>{path}</code></li>)}</ul>}
      </section>}
      <label className="maintenance-confirm"><input type="checkbox" checked={accepted} disabled={busy} onChange={event => setAccepted(event.target.checked)}/><span>{pt ? "Conferi os caminhos e entendo que esta remoção é permanente." : "I reviewed the paths and understand this removal is permanent."}</span></label>
    </NexusPopup>}
  </section>;
}

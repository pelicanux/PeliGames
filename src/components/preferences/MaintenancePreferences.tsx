import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nContext";
import { MenuIcon } from "../MenuIcon";
import { NexusPopup } from "../../modules/nexus/NexusPopup";
import { notifyError } from "../../services/notifications";
import "./MaintenancePreferences.css";

type ModGame = { id: string; name: string; directory: string; device: string; available: boolean; portable: boolean; mods: number };
type Plan = { action: "reset" | "uninstall"; cleanup_data: boolean; paths: string[]; mod_games?: ModGame[]; installed_games?: { name: string; directory: string; available: boolean }[]; mod_revision?: string; pelirun?: string[] };

function KeepOption({ title, description, checked, disabled, onChange }: { title: string; description: string; checked: boolean; disabled: boolean; onChange: (checked: boolean) => void }) {
  return <label className="maintenance-keep-option"><span><strong>{title}</strong><small>{description}</small></span><input type="checkbox" checked={checked} disabled={disabled} onChange={event => onChange(event.target.checked)} /></label>;
}

export function MaintenancePreferences({ uninstall = false }: { uninstall?: boolean }) {
  const pt = useI18n().language === "pt";
  const [plan, setPlan] = useState<Plan | null>(null);
  const [busy, setBusy] = useState(false);
  const [accepted, setAccepted] = useState(false);
  const [removeMods, setRemoveMods] = useState(false);
  const [preserveAccounts, setPreserveAccounts] = useState(false);
  const [preserveGames, setPreserveGames] = useState(false);
  const [preserveMangohud, setPreserveMangohud] = useState(false);
  const [removePelirun, setRemovePelirun] = useState(false);
  const [previewCompleted, setPreviewCompleted] = useState(false);
  const lock = useRef(false);
  const title = uninstall ? (pt ? "Desinstalar AppImage" : "Uninstall AppImage") : (pt ? "Redefinir PeliGames" : "Reset PeliGames");
  const description = uninstall
    ? (pt ? "Remova o aplicativo e escolha se também deseja limpar seus dados." : "Remove the application and choose whether to clear its data too.")
    : (pt ? "Restaure as configurações do launcher e escolha o que deseja manter." : "Reset launcher settings and choose what you want to keep.");
  const cleaning = !uninstall || Boolean(plan?.cleanup_data);
  const actionLabel = uninstall ? (pt ? "Desinstalar e fechar" : "Uninstall and close") : (pt ? "Redefinir e fechar" : "Reset and close");
  const blockedGame = cleaning ? plan?.mod_games?.find(game => !game.available && (removeMods || !game.portable)) : undefined;
  const review = async (cleanupData = false) => {
    if (lock.current) return;
    lock.current = true; setBusy(true);
    try {
      setPlan(await invoke<Plan>("launcher_maintenance_plan", { action: uninstall ? "uninstall" : "reset", cleanupData }));
      setAccepted(false); setRemoveMods(false); setPreserveAccounts(false); setPreserveGames(false); setPreserveMangohud(false); setRemovePelirun(false);
    } catch (error) { notifyError(title, String(error)); }
    finally { lock.current = false; setBusy(false); }
  };
  const apply = async () => {
    if (!plan || !accepted || blockedGame || lock.current) return;
    lock.current = true; setBusy(true);
    try {
      if ("__PELI_UI_PREVIEW__" in window) {
        setPreviewCompleted(true);
      } else {
        await invoke("launcher_maintenance_apply", { action: plan.action, cleanupData: plan.cleanup_data, confirmedPaths: plan.paths, removeMods, preserveAccounts, preserveGames, preserveMangohud, confirmedModRevision: plan.mod_revision, removePelirun });
      }
    } catch (error) { notifyError(title, String(error)); }
    finally { lock.current = false; setBusy(false); }
  };
  if (previewCompleted) return <NexusPopup className="steam-manual-popup maintenance-popup" title={pt ? "Demonstração concluída" : "Preview complete"} titleIcon={<MenuIcon name="check" />} onClose={() => { setPreviewCompleted(false); setPlan(null); }} footer={<button type="button" className="btn btn-secondary" onClick={() => { setPreviewCompleted(false); setPlan(null); }}>{pt ? "Voltar" : "Back"}</button>}>
    <div className="maintenance-completion-copy"><MenuIcon name="check" /><h3>{pt ? "O PeliGames fecharia agora" : "PeliGames would close now"}</h3><p>{pt ? "Na versão instalada, a operação é concluída após fechar o launcher, sem abri-lo novamente." : "In the installed version, the operation completes after closing the launcher, without reopening it."}</p><small>{pt ? "Esta prévia não remove arquivos, não encerra programas e não altera suas contas." : "This preview does not remove files, close programs or change your accounts."}</small></div>
  </NexusPopup>;
  return <section className="preference-section" data-preference-id={uninstall ? "appimage-uninstall" : "launcher-reset"}>
    <h3>{title}</h3><p>{description}</p>
    <button type="button" className="preference-outline danger" disabled={busy} onClick={() => void review()}><MenuIcon name={uninstall ? "trash" : "repair"} />{title}</button>
    {plan && <NexusPopup className="steam-manual-popup maintenance-popup" title={title} titleIcon={<MenuIcon name={uninstall ? "trash" : "repair"} />} onClose={() => { if (!busy) setPlan(null); }} footer={<>
      <label className="maintenance-confirm maintenance-final-confirm"><input type="checkbox" checked={accepted} disabled={busy} onChange={event => setAccepted(event.target.checked)} /><span>{cleaning ? (pt ? "Entendo que os dados não preservados serão apagados." : "I understand that data I do not keep will be deleted.") : (pt ? "Entendo que o aplicativo e os atalhos serão removidos." : "I understand the application and shortcuts will be removed.")}</span></label>
      <div className="maintenance-actions"><button type="button" className="btn btn-secondary" disabled={busy} onClick={() => setPlan(null)}>{pt ? "Cancelar" : "Cancel"}</button><button type="button" className="btn btn-danger" disabled={busy || !accepted || Boolean(blockedGame)} aria-busy={busy} onClick={() => void apply()}>{busy ? (pt ? "Aguarde…" : "Please wait…") : actionLabel}</button></div>
    </>}>
      <div className="maintenance-intro"><MenuIcon name="info" /><p>{uninstall && !cleaning ? (pt ? "O PeliGames será fechado para remover esta instalação. Seus jogos, contas e configurações permanecem." : "PeliGames will close to remove this installation. Your games, accounts and settings remain.") : (pt ? "Preferências, histórico, capas e cache serão apagados, exceto o que você escolher manter. Feche jogos, instalações e downloads. O PeliGames fechará ao confirmar." : "Preferences, history, covers and cache will be cleared, except what you choose to keep. Close games, installations and downloads. PeliGames will close when you confirm.")}</p></div>
      {uninstall && <section className="maintenance-uninstall-choice"><KeepOption title={pt ? "Limpar também os dados do PeliGames" : "Also clear PeliGames data"} description={pt ? "Inclui configurações, contas, histórico, capas e cache. Escolha abaixo o que deseja preservar." : "Includes settings, accounts, history, covers and cache. Choose below what you want to keep."} checked={Boolean(plan.cleanup_data)} disabled={busy} onChange={value => { setAccepted(false); void review(value); }} /></section>}
      {cleaning && <section className="maintenance-preservation"><h3>{pt ? "O que manter" : "What to keep"}</h3><p className="maintenance-section-note">{pt ? "Marque os dados que deseja manter no computador." : "Select the data you want to keep on this computer."}</p>
        <KeepOption title={pt ? "Jogos do Pelinstall" : "Pelinstall games"} description={pt ? "Mantém os registros dos jogos, inclusive em discos desconectados. Runners precisarão ser reinstalados." : "Keeps game registrations, including disconnected disks. Runners will need reinstalling."} checked={preserveGames} disabled={busy} onChange={value => { setPreserveGames(value); setAccepted(false); }} />
        {preserveGames && Boolean(plan.installed_games?.length) && <details className="maintenance-details maintenance-row-details"><summary>{pt ? `Ver jogos registrados (${plan.installed_games!.length})` : `View registered games (${plan.installed_games!.length})`}</summary><ul className="maintenance-paths maintenance-game-list" tabIndex={0}>{plan.installed_games!.map(game => <li key={game.directory}><strong>{game.name}</strong><code>{game.directory}</code>{!game.available && <small>{pt ? "Indisponível neste momento. O registro será preservado." : "Currently unavailable. Its registration will be kept."}</small>}</li>)}</ul></details>}
        <KeepOption title={pt ? "Contas conectadas" : "Connected accounts"} description={pt ? "Mantém as sessões e credenciais do Matrix, Discord e Nexus Mods." : "Keeps Matrix, Discord and Nexus Mods sessions and credentials."} checked={preserveAccounts} disabled={busy} onChange={value => { setPreserveAccounts(value); setAccepted(false); }} />
        <KeepOption title={pt ? "Perfis do MangoHUD" : "MangoHUD profiles"} description={pt ? "Mantém o perfil atual e os presets personalizados. Os arquivos do componente não são preservados." : "Keeps your current profile and custom presets. Component files are not kept."} checked={preserveMangohud} disabled={busy} onChange={value => { setPreserveMangohud(value); setAccepted(false); }} />
        <KeepOption title={pt ? "Mods do Nexus" : "Nexus mods"} description={removeMods ? (pt ? "Os mods gerenciados serão removidos e os arquivos originais restaurados. Jogos e saves permanecem." : "Managed mods will be removed and original files restored. Games and saves remain.") : (pt ? "Mantém os mods, pacotes e registros junto de cada jogo para recuperar o gerenciamento." : "Keeps mods, packages and records beside each game to recover mod management.")} checked={!removeMods} disabled={busy} onChange={value => { setRemoveMods(!value); setAccepted(false); }} />
        {Boolean(plan.mod_games?.length) && <details className="maintenance-details maintenance-row-details"><summary>{pt ? `Ver jogos com mods (${plan.mod_games!.length})` : `View games with mods (${plan.mod_games!.length})`}</summary><ul className="maintenance-paths maintenance-game-list" tabIndex={0}>{plan.mod_games!.map(game => <li key={game.id}><strong>{game.name} · {game.mods} mods</strong><code>{game.directory}</code>{!game.available && <small>{pt ? `Indisponível. Dispositivo: ${game.device}.` : `Unavailable. Device: ${game.device}.`}</small>}</li>)}</ul></details>}
      </section>}
      {blockedGame && <p className="maintenance-blocked" role="alert">{pt ? `Conecte e monte o dispositivo de “${blockedGame.name}” no caminho original antes de continuar. Os arquivos e registros precisam estar acessíveis para esta operação.` : `Connect and mount the device for “${blockedGame.name}” at its original path before continuing. Its files and records must be accessible for this operation.`}</p>}
      {cleaning && <details className="maintenance-details maintenance-advanced"><summary>{pt ? "Opções avançadas" : "Advanced options"}</summary><KeepOption title={pt ? "Remover também o PeliRun" : "Also remove PeliRun"} description={!plan.pelirun?.length ? (pt ? "O auxiliar não está instalado neste computador." : "The helper is not installed on this computer.") : (pt ? "Remove o auxiliar que aplica ajustes ao iniciar jogos por outros launchers. Feche a Steam antes de remover." : "Removes the helper that applies settings when launching games through other launchers. Close Steam before removing it.")} checked={removePelirun} disabled={busy || !plan.pelirun?.length} onChange={value => { setRemovePelirun(value); setAccepted(false); }} />
        <p className="maintenance-section-note">{pt ? "Se mantido, o PeliRun continua permitindo abrir os jogos, mas seus perfis de configuração serão redefinidos. Se removido, chamadas copiadas manualmente para outros launchers também precisam ser retiradas." : "If kept, PeliRun still lets games launch, but its configuration profiles are reset. If removed, calls manually copied to other launchers also need removing."}</p>
        {removePelirun && Boolean(plan.pelirun?.length) && <ul className="maintenance-paths">{plan.pelirun!.map(path => <li key={path}><code>{path}</code></li>)}</ul>}
      </details>}
      <details className="maintenance-details maintenance-operation-paths"><summary>{pt ? `Pastas ${uninstall ? "da desinstalação" : "da redefinição"} (${plan.paths.length})` : `Folders affected (${plan.paths.length})`}</summary><ul className="maintenance-paths">{plan.paths.map(path => <li key={path}><code>{path}</code></li>)}</ul></details>
      <p className="maintenance-recovery-note">{pt ? "A operação termina com o PeliGames fechado. Ele só abre novamente se você iniciar manualmente. Jogos externos e saves não são apagados." : "The operation finishes with PeliGames closed. It only opens again if you start it manually. External games and saves are not deleted."}</p>
    </NexusPopup>}
  </section>;
}

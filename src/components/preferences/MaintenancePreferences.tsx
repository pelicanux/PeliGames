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
  const {t: tx} = useI18n();

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
  const title = uninstall ? (tx("ui-maintenance-preferences", "uninstall_appimage")) : (tx("ui-maintenance-preferences", "reset_peligames"));
  const description = uninstall
    ? (tx("ui-maintenance-preferences", "remove_the_application_and_choose_whether_to_clear_its_data_too"))
    : (tx("ui-maintenance-preferences", "reset_launcher_settings_and_choose_what_you_want_to_keep"));
  const cleaning = !uninstall || Boolean(plan?.cleanup_data);
  const actionLabel = uninstall ? (tx("ui-maintenance-preferences", "uninstall_and_close")) : (tx("ui-maintenance-preferences", "reset_and_close"));
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
  if (previewCompleted) return <NexusPopup className="steam-manual-popup maintenance-popup" title={tx("ui-maintenance-preferences", "preview_complete")} titleIcon={<MenuIcon name="check" />} onClose={() => { setPreviewCompleted(false); setPlan(null); }} footer={<button type="button" className="btn btn-secondary" onClick={() => { setPreviewCompleted(false); setPlan(null); }}>{tx("ui-maintenance-preferences", "back")}</button>}>
    <div className="maintenance-completion-copy"><MenuIcon name="check" /><h3>{tx("ui-maintenance-preferences", "peligames_would_close_now")}</h3><p>{tx("ui-maintenance-preferences", "in_the_installed_version_the_operation_completes_after_closing_the_launcher_without_reop")}</p><small>{tx("ui-maintenance-preferences", "this_preview_does_not_remove_files_close_programs_or_change_your_accounts")}</small></div>
  </NexusPopup>;
  return <section className="preference-section" data-preference-id={uninstall ? "appimage-uninstall" : "launcher-reset"}>
    <h3>{title}</h3><p>{description}</p>
    <button type="button" className="preference-outline danger" disabled={busy} onClick={() => void review()}><MenuIcon name={uninstall ? "trash" : "repair"} />{title}</button>
    {plan && <NexusPopup className="steam-manual-popup maintenance-popup" title={title} titleIcon={<MenuIcon name={uninstall ? "trash" : "repair"} />} onClose={() => { if (!busy) setPlan(null); }} footer={<>
      <label className="maintenance-confirm maintenance-final-confirm"><input type="checkbox" checked={accepted} disabled={busy} onChange={event => setAccepted(event.target.checked)} /><span>{cleaning ? (tx("ui-maintenance-preferences", "i_understand_that_data_i_do_not_keep_will_be_deleted")) : (tx("ui-maintenance-preferences", "i_understand_the_application_and_shortcuts_will_be_removed"))}</span></label>
      <div className="maintenance-actions"><button type="button" className="btn btn-secondary" disabled={busy} onClick={() => setPlan(null)}>{tx("ui-maintenance-preferences", "cancel")}</button><button type="button" className="btn btn-danger" disabled={busy || !accepted || Boolean(blockedGame)} aria-busy={busy} onClick={() => void apply()}>{busy ? (tx("ui-maintenance-preferences", "please_wait")) : actionLabel}</button></div>
    </>}>
      <div className="maintenance-intro"><MenuIcon name="info" /><p>{uninstall && !cleaning ? (tx("ui-maintenance-preferences", "peligames_will_close_to_remove_this_installation_your_games_accounts_and_settings_remain")) : (tx("ui-maintenance-preferences", "preferences_history_covers_and_cache_will_be_cleared_except_what_you_choose_to_keep_clos"))}</p></div>
      {uninstall && <section className="maintenance-uninstall-choice"><KeepOption title={tx("ui-maintenance-preferences", "also_clear_peligames_data")} description={tx("ui-maintenance-preferences", "includes_settings_accounts_history_covers_and_cache_choose_below_what_you_want_to_keep")} checked={Boolean(plan.cleanup_data)} disabled={busy} onChange={value => { setAccepted(false); void review(value); }} /></section>}
      {cleaning && <section className="maintenance-preservation"><h3>{tx("ui-maintenance-preferences", "what_to_keep")}</h3><p className="maintenance-section-note">{tx("ui-maintenance-preferences", "select_the_data_you_want_to_keep_on_this_computer")}</p>
        <KeepOption title={tx("ui-maintenance-preferences", "pelinstall_games")} description={tx("ui-maintenance-preferences", "keeps_game_registrations_including_disconnected_disks_runners_will_need_reinstalling")} checked={preserveGames} disabled={busy} onChange={value => { setPreserveGames(value); setAccepted(false); }} />
        {preserveGames && Boolean(plan.installed_games?.length) && <details className="maintenance-details maintenance-row-details"><summary>{tx("ui-maintenance-preferences", "view_registered_games", {"plan_installed_games_length": plan.installed_games!.length})}</summary><ul className="maintenance-paths maintenance-game-list" tabIndex={0}>{plan.installed_games!.map(game => <li key={game.directory}><strong>{game.name}</strong><code>{game.directory}</code>{!game.available && <small>{tx("ui-maintenance-preferences", "currently_unavailable_its_registration_will_be_kept")}</small>}</li>)}</ul></details>}
        <KeepOption title={tx("ui-maintenance-preferences", "connected_accounts")} description={tx("ui-maintenance-preferences", "keeps_matrix_discord_and_nexus_mods_sessions_and_credentials")} checked={preserveAccounts} disabled={busy} onChange={value => { setPreserveAccounts(value); setAccepted(false); }} />
        <KeepOption title={tx("ui-maintenance-preferences", "mangohud_profiles")} description={tx("ui-maintenance-preferences", "keeps_your_current_profile_and_custom_presets_component_files_are_not_kept")} checked={preserveMangohud} disabled={busy} onChange={value => { setPreserveMangohud(value); setAccepted(false); }} />
        <KeepOption title={tx("ui-maintenance-preferences", "nexus_mods")} description={removeMods ? (tx("ui-maintenance-preferences", "managed_mods_will_be_removed_and_original_files_restored_games_and_saves_remain")) : (tx("ui-maintenance-preferences", "keeps_mods_packages_and_records_beside_each_game_to_recover_mod_management"))} checked={!removeMods} disabled={busy} onChange={value => { setRemoveMods(!value); setAccepted(false); }} />
        {Boolean(plan.mod_games?.length) && <details className="maintenance-details maintenance-row-details"><summary>{tx("ui-maintenance-preferences", "view_games_with_mods", {"plan_mod_games_length": plan.mod_games!.length})}</summary><ul className="maintenance-paths maintenance-game-list" tabIndex={0}>{plan.mod_games!.map(game => <li key={game.id}><strong>{game.name} · {game.mods} mods</strong><code>{game.directory}</code>{!game.available && <small>{tx("ui-maintenance-preferences", "unavailable_device", {"game_device": game.device})}</small>}</li>)}</ul></details>}
      </section>}
      {blockedGame && <p className="maintenance-blocked" role="alert">{tx("ui-maintenance-preferences", "connect_and_mount_the_device_for_at_its_original_path_before_continuing_its_files_and_re", {"blockedgame_name": blockedGame.name})}</p>}
      {cleaning && <details className="maintenance-details maintenance-advanced"><summary>{tx("ui-maintenance-preferences", "advanced_options")}</summary><KeepOption title={tx("ui-maintenance-preferences", "also_remove_pelirun")} description={!plan.pelirun?.length ? (tx("ui-maintenance-preferences", "the_helper_is_not_installed_on_this_computer")) : (tx("ui-maintenance-preferences", "removes_the_helper_that_applies_settings_when_launching_games_through_other_launchers_cl"))} checked={removePelirun} disabled={busy || !plan.pelirun?.length} onChange={value => { setRemovePelirun(value); setAccepted(false); }} />
        <p className="maintenance-section-note">{tx("ui-maintenance-preferences", "if_kept_pelirun_still_lets_games_launch_but_its_configuration_profiles_are_reset_if_remo")}</p>
        {removePelirun && Boolean(plan.pelirun?.length) && <ul className="maintenance-paths">{plan.pelirun!.map(path => <li key={path}><code>{path}</code></li>)}</ul>}
      </details>}
      <details className="maintenance-details maintenance-operation-paths"><summary>{tx("ui-maintenance-preferences", "affected_folders", {count: plan.paths.length})}</summary><ul className="maintenance-paths">{plan.paths.map(path => <li key={path}><code>{path}</code></li>)}</ul></details>
      <p className="maintenance-recovery-note">{tx("ui-maintenance-preferences", "the_operation_finishes_with_peligames_closed_it_only_opens_again_if_you_start_it_manuall")}</p>
    </NexusPopup>}
  </section>;
}

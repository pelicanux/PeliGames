import {useI18n} from "../../i18n/I18nContext";
import { useState, type ReactNode } from "react";
import { openBrowserUrl } from "../../services/browser";
import { ModOptionSwitch } from "../../components/ModOptionRow";
import { ConfirmModal } from "../../components/ConfirmModal";
import { FeatureReportSection } from "../reports/FeatureReportSection";
import { featureReportContext } from "../reports/reportContext";
import { openDanielPreferences } from "./danielNr";
import { useDanielNr, type DanielPanelProps } from "./useDanielNr";
import { DanielRuntimeVersion } from "./DanielRuntimeVersion";
import { DanielLoadingDll } from "./DanielLoadingDll";
import { DanielGameGuide } from "./DanielGameGuide";
import "./danielNrTest.css";

function Step({ title, children }: { title: string; children: ReactNode }) {
  return <section className="daniel-nr-step"><header><h4>{title}</h4></header><div className="daniel-nr-step-content">{children}</div></section>;
}

export function DanielNrTestPanel(props: DanielPanelProps) {
  const {t: tx} = useI18n();

  const { game, running = false } = props;
  const panel = useDanielNr(props);
  const { state, request, blocked, loading, packageReady, selectedRuntime, unsupported, supportedProfile } = panel;
  const [confirmRemove, setConfirmRemove] = useState(false);
  return <FeatureReportSection label="Daniel NR (experimental)" context={featureReportContext(game, "daniel-nr", request.directory, state?.directory || "")}>
    <div className="modern-dlss-panels daniel-nr-workflow">
      <section className="modern-dlss-card daniel-nr-test" aria-label={tx("ui-daniel-nr-test-panel", "daniel_nr_installation")}>
        <header className="daniel-nr-test-heading"><h3>Daniel NR</h3><span className="modern-meta-chip">Experimental</span></header>
        <p className="modern-dlss-description">{tx("ui-daniel-nr-test-panel", "follow_the_steps_to_prepare_the_environment_install_for_this_game_and_adjust_the_image_w")}</p>
        {unsupported ? <p className="modern-dlss-description">{game.flatpak ? (tx("ui-daniel-nr-test-panel", "flatpak_launchers_are_not_supported_in_this_test_yet")) : (tx("ui-daniel-nr-test-panel", "select_the_windows_game_version_to_use_proton"))}</p> : <>
          {loading && <p role="status">{tx("ui-daniel-nr-test-panel", "checking_installation")}</p>}
          {!state && !loading && <div className="modern-dlss-actions"><button type="button" className="modern-mini-btn" disabled={blocked} onClick={() => void panel.chooseExecutable()}>{tx("ui-daniel-nr-test-panel", "choose_executable")}</button><button type="button" className="modern-mini-btn" disabled={blocked} onClick={() => void panel.reload()}>{tx("ui-daniel-nr-test-panel", "retry")}</button></div>}
          {state && <>
            {packageReady || state.installed ? <div className="daniel-nr-environment-link"><span>{tx("ui-daniel-nr-test-panel", "environment_ready")}</span><button type="button" className="modern-mini-btn" disabled={blocked} onClick={openDanielPreferences}>{tx("ui-daniel-nr-test-panel", "manage_add_ons")}</button></div> : <Step title={tx("ui-daniel-nr-test-panel", "prepare_the_environment")}>
              <p>{tx("ui-daniel-nr-test-panel", "prepare_package_in_settings_add_ons_daniel_nr_preparation_is_shared_across_games", {"selectedruntime": selectedRuntime})}</p>
              <button type="button" className="modern-dlss-primary" disabled={blocked} onClick={openDanielPreferences}>{tx("ui-daniel-nr-test-panel", "prepare_environment")}</button>
            </Step>}
            <Step title={state.installed ? (tx("ui-daniel-nr-test-panel", "installed_for_this_game")) : (tx("ui-daniel-nr-test-panel", "install_for_this_game"))}>
              <p>{tx("ui-daniel-nr-test-panel", "requires_a_64_bit_windows_game_directx_12_a_compatible_radeon_and_wine_11_based_proton")}</p>
              <details className="daniel-nr-destination"><summary>{tx("ui-daniel-nr-test-panel", "executable_and_destination")}</summary><p className="daniel-nr-path">{state.executable}</p><p>{tx("ui-daniel-nr-test-panel", "choose_the_executable_that_renders_the_game_unreal_games_usually_use_binaries_win64_the_")}</p>{!state.installed && <button type="button" className="modern-mini-btn" disabled={blocked} onClick={() => void panel.chooseExecutable()}>{tx("ui-daniel-nr-test-panel", "change_executable")}</button>}</details>
              {state.conflict && <p className="daniel-nr-test-error" role="alert">{state.conflict}</p>}
              {!state.gpu_access && <p className="daniel-nr-test-error">{tx("ui-daniel-nr-test-panel", "no_gpu_access_through_dev_kfd_check_your_distribution_s_permissions")}</p>}
              {!supportedProfile && <p>{tx("ui-daniel-nr-test-panel", "register_this_game_in_the_main_library_to_configure_its_launch")}</p>}
              {!state.installed && <>
                <label className="daniel-nr-confirm"><input type="checkbox" checked={panel.dx12} disabled={blocked || !packageReady} onChange={e => panel.setDx12(e.target.checked)}/>{tx("ui-daniel-nr-test-panel", "the_game_is_configured_for_directx_12")}</label>
                <div className="modern-dlss-actions"><button type="button" className="modern-dlss-primary" disabled={blocked || !packageReady || state.pending_version_change || !panel.dx12 || Boolean(state.conflict) || !state.gpu_access || !supportedProfile || !state.loading_dll.options.some(o => o.available && (panel.proxy === "auto" || o.name === panel.proxy))} onClick={() => void panel.execute("install")}>{tx("ui-daniel-nr-test-panel", "install_and_enable")}</button></div>
              </>}
              {state.installed && !panel.linked && <button type="button" className="modern-mini-btn" disabled={blocked || !supportedProfile || state.pending_version_change} onClick={() => void panel.execute("link")}>{tx("ui-daniel-nr-test-panel", "complete_launch_integration")}</button>}
            </Step>
            <Step title={tx("ui-daniel-nr-test-panel", "launch_the_game_and_adjust_the_image")}>
              <DanielGameGuide version={selectedRuntime}/>
            </Step>
          </>}
        </>}
        {running && <p role="status" className="modern-dlss-description daniel-nr-test-status">{tx("ui-daniel-nr-test-panel", "game_running_close_it_to_change_the_installation_or_options")}</p>}
        {panel.progress && <p role="status" className="modern-dlss-description daniel-nr-test-status">{panel.progress}</p>}
        {panel.error && <p className="daniel-nr-test-error" role="alert">{panel.error}</p>}
      </section>
      {!unsupported && <section className="modern-dlss-card daniel-nr-test" aria-label={tx("ui-daniel-nr-test-panel", "daniel_nr_options")}>
        <h3>{tx("ui-daniel-nr-test-panel", "options_for_this_game")}</h3>
        <p className="modern-dlss-description">{state?.installed ? (tx("ui-daniel-nr-test-panel", "selections_apply_automatically_on_the_next_launch_image_settings_remain_in_the_in_game_p")) : (tx("ui-daniel-nr-test-panel", "choose_the_version_and_dll_before_installing_after_installation_selecting_another_option"))}</p>
        {state && <>
          <DanielRuntimeVersion state={state} value={selectedRuntime} onChange={panel.selectRuntime} onRecover={() => void panel.execute("recover-version")} disabled={blocked}/>
          <DanielLoadingDll installed={state.installed} state={state.loading_dll} value={panel.proxy} onChange={panel.selectProxy} disabled={blocked || state.pending_version_change}/>
          <div className="modern-dlss-option"><div><span>{tx("ui-daniel-nr-test-panel", "start_enabled")}</span><p>{tx("ui-daniel-nr-test-panel", "starts_neural_rendering_with_the_game_disabling_keeps_the_mod_installed")}</p></div><ModOptionSwitch label={tx("ui-daniel-nr-test-panel", "start_with_daniel_nr_enabled")} enabled={state.installed && state.enabled} disabled={blocked || !state.installed || state.pending_version_change || !supportedProfile || (!state.enabled && Boolean(state.conflict))} onToggle={() => void panel.execute("toggle")}/></div>
          {state.installed && <div className="daniel-nr-uninstall"><div><span>{tx("ui-daniel-nr-test-panel", "uninstall_from_this_game")}</span><p>{tx("ui-daniel-nr-test-panel", "restores_original_files_and_backs_up_settings_shared_components_remain_available")}</p></div><button type="button" className="modern-mini-btn" disabled={blocked || state.pending_version_change || !supportedProfile} onClick={() => setConfirmRemove(true)}>{tx("ui-daniel-nr-test-panel", "uninstall")}</button></div>}
        </>}
        <details className="daniel-nr-support"><summary>{tx("ui-daniel-nr-test-panel", "diagnostics_and_support")}</summary>
          <p className="modern-dlss-description">{tx("ui-daniel-nr-test-panel", "check_picture_and_stability_an_open_menu_does_not_prove_neural_processing_log_counters_h")}</p>
          <div className="modern-dlss-actions"><button type="button" className="modern-mini-btn" disabled={panel.busy || !state} onClick={panel.diagnose}>{tx("ui-daniel-nr-test-panel", "view_diagnostics")}</button><button type="button" className="modern-mini-btn" disabled={panel.busy} onClick={() => void panel.reload()}>{tx("ui-daniel-nr-test-panel", "check_again")}</button><button type="button" className="modern-mini-btn" onClick={() => void openBrowserUrl("https://github.com/danielblnc/DLSS-NR-on-AMD").catch(panel.fail)}>{tx("ui-daniel-nr-test-panel", "daniel_s_project")}</button></div>
          {panel.diagnostics !== undefined && <pre className="daniel-nr-diagnostics" tabIndex={0}>{panel.diagnostics}</pre>}
        </details>
      </section>}
    </div>
    {confirmRemove && <ConfirmModal title={tx("ui-daniel-nr-test-panel", "uninstall_daniel_nr_from_this_game")} message={tx("ui-daniel-nr-test-panel", "the_launch_integration_will_be_removed_and_original_files_restored_edited_settings_will_")} confirmText={tx("ui-daniel-nr-test-panel", "uninstall")} cancelText={tx("ui-daniel-nr-test-panel", "cancel")} onCancel={() => setConfirmRemove(false)} onConfirm={() => { setConfirmRemove(false); void panel.execute("remove"); }}/>}
  </FeatureReportSection>;
}

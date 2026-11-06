import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../../i18n/I18nContext";
import { ConfirmModal } from "../../components/ConfirmModal";
import { MenuIcon } from "../../components/MenuIcon";
import { SelectionMenu } from "../../components/SelectionMenu";
import { openDirectoryPicker, openFilePicker } from "../../services/tauriService";
import { DANIEL_COMPONENT_CHANGED, loadDanielComponent, operateDanielComponent, type DanielComponentState } from "./danielNr";
import "./danielEnvironment.css";

export function DanielEnvironmentPreferences({ disabled, onBusy }: { disabled: boolean; onBusy: (value: boolean) => void }) {
  const {t: tx} = useI18n();

  const [state, setState] = useState<DanielComponentState>();
  const [version, setVersion] = useState("0.6.0");
  const [removal, setRemoval] = useState<string | null>(null);
  const [compiler, setCompiler] = useState("");
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [progress, setProgress] = useState("");
  const mounted = useRef(true), operation = useRef(false);
  const blocked = disabled || busy || Boolean(removal);
  const selected = state?.runtimes.find(r => r.version === version);
  const ready = selected?.ready;
  const reload = async () => {
    try { const next = await loadDanielComponent(); if (mounted.current) setState(next); }
    catch (e) { if (mounted.current) setError(String(e)); }
  };
  useEffect(() => {
    mounted.current = true; void reload();
    const subscription = listen<{ directory: string; message: string }>("danielNrProgress", event => {
      if (mounted.current && operation.current && event.payload.directory === "") setProgress(event.payload.message);
    });
    void subscription.catch(() => {});
    return () => { mounted.current = false; void subscription.then(off => off()).catch(() => {}); };
  }, []);
  const run = async (removeVersion?: string) => {
    if (operation.current || (!removeVersion && blocked) || !state) return;
    operation.current = true; setBusy(true); onBusy(true); setError(""); setProgress("");
    try {
      const source = !removeVersion && version === "0.8.0" ? await openDirectoryPicker(tx("ui-daniel-environment-preferences", "prepared_daniel_0_8_package")) : undefined;
      if (!removeVersion && version === "0.8.0" && !source) return;
      const next = await operateDanielComponent(removeVersion ? "remove-package" : version === "0.8.0" ? "import-runtime" : "prepare", compiler, removeVersion || version, source || undefined);
      if (mounted.current) { setState(next); setProgress(removeVersion ? (tx("ui-daniel-environment-preferences", "package_uninstalled_game_installations_were_preserved", {"removeversion": removeVersion})) : (tx("ui-daniel-environment-preferences", "environment_ready_return_to_the_game_to_install_this_version"))); }
      window.dispatchEvent(new Event(DANIEL_COMPONENT_CHANGED));
    } catch (e) { if (mounted.current) { setError(String(e)); setProgress(""); } }
    finally { operation.current = false; if (mounted.current) { setBusy(false); setRemoval(null); } onBusy(false); }
  };
  const requestRemoval = () => { if (!blocked) { setRemoval(version); onBusy(true); } };
  const closeRemoval = () => { if (!operation.current) { setRemoval(null); onBusy(false); } };
  const browseCompiler = async () => {
    try { const file = await openFilePicker("d3dcompiler_47.dll", ["dll"]); if (file && mounted.current) setCompiler(file); }
    catch (e) { if (mounted.current) setError(String(e)); }
  };
  return <section data-preference-id="runtime-daniel" className="mod-workspace daniel-component-workspace" aria-label={tx("ui-daniel-environment-preferences", "daniel_nr_environment")}>
    <header className="mod-intro"><h3>{tx("ui-daniel-environment-preferences", "prepare_daniel_nr")}</h3><p>{tx("ui-daniel-environment-preferences", "prepare_the_neural_component_here_then_choose_which_games_use_it_packages_are_shared_acr")}</p></header>
    <section className="mod-install-surface" aria-label={tx("ui-daniel-environment-preferences", "daniel_nr_component_preparation")} aria-busy={busy}>
      <div className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="neural"/></span><div><h4>Daniel NR</h4><p>{tx("ui-daniel-environment-preferences", "neural_rendering_experimental_component")}</p></div><span className="mod-status-badge" data-ready={Boolean(ready)} role="status">{!state ? (tx("ui-daniel-environment-preferences", "checking")) : ready ? (tx("ui-daniel-environment-preferences", "ready")) : (tx("ui-daniel-environment-preferences", "not_prepared"))}</span></div>
      {state && <>
        <div className="mod-setting-row"><div className="mod-row-copy"><strong>{tx("ui-daniel-environment-preferences", "runtime_version")}</strong><small>{tx("ui-daniel-environment-preferences", "each_version_has_its_own_package_choose_the_version_for_each_game_in_the_library")}</small></div><SelectionMenu label={tx("ui-daniel-environment-preferences", "version_to_prepare")} value={version} options={state.runtimes.map(r => ({ value: r.version, label: r.version }))} onChange={value => { setVersion(value); setError(""); setProgress(""); }} disabled={blocked}/></div>
        {!ready && (version === "0.6.0" ? <>
          <div className="mod-setting-row daniel-compiler-row"><div className="mod-row-copy"><strong>{tx("ui-daniel-environment-preferences", "shader_compiler")}</strong><small>{tx("ui-daniel-environment-preferences", "select_a_native_64_bit_d3dcompiler_47_dll_from_a_game_you_own")}</small></div><div className="daniel-compiler-controls"><input type="text" aria-label="d3dcompiler_47.dll" value={compiler} disabled={blocked} onChange={e => setCompiler(e.target.value)} placeholder={tx("ui-daniel-environment-preferences", "native_dll_path")}/><button type="button" className="mod-action-secondary" disabled={blocked} onClick={() => void browseCompiler()}><MenuIcon name="folderSearch"/>{tx("ui-daniel-environment-preferences", "browse")}</button></div></div>
          {!state.python_ready && <p className="mod-feedback-error" role="alert">{tx("ui-daniel-environment-preferences", "requires_python_3_12_for_3_12_3_13_install_your_distribution_s_zstandard_module_as_well")}</p>}
        </> : <div className="mod-setting-row"><div className="mod-row-copy"><strong>{tx("ui-daniel-environment-preferences", "experimental_0_8_0_package")}</strong><small>{tx("ui-daniel-environment-preferences", "import_the_prepared_package_folder_it_already_includes_d3dcompiler_47_dll_no_separate_se")}</small></div></div>)}
        {!ready && <div className="mod-step-actions"><button type="button" className="mod-action-primary" disabled={blocked || (version === "0.6.0" && (!compiler || !state.python_ready))} onClick={() => void run()}><MenuIcon name={version === "0.8.0" ? "folder" : "download"}/>{busy ? (tx("ui-daniel-environment-preferences", "preparing")) : version === "0.8.0" ? (tx("ui-daniel-environment-preferences", "import_0_8_0_package")) : (tx("ui-daniel-environment-preferences", "prepare_environment"))}</button></div>}
        {selected?.present && <div className="mod-setting-row"><div className="mod-row-copy"><strong>{tx("ui-daniel-environment-preferences", "package", {"version": version})}</strong><small>{tx("ui-daniel-environment-preferences", "removes_only_this_launcher_package_keeps_the_other_version_settings_and_copies_already_i")}</small></div><button type="button" className="mod-action-secondary" disabled={blocked} onClick={requestRemoval}><MenuIcon name="trash"/>{tx("ui-daniel-environment-preferences", "uninstall", {"version": version})}</button></div>}
      </>}
      {!state && error && <div className="mod-step-actions"><button className="mod-action-secondary" type="button" disabled={blocked} onClick={() => void reload()}><MenuIcon name="repair"/>{tx("ui-daniel-environment-preferences", "retry")}</button></div>}
      {progress && <p role="status" className="mod-inline-progress">{progress}</p>}
      {error && <p role="alert" className="mod-feedback-error">{error}</p>}
    </section>
    {removal && <ConfirmModal title={tx("ui-daniel-environment-preferences", "uninstall_daniel_package", {"removal": removal})} message={tx("ui-daniel-environment-preferences", "only_prepared_package_will_be_removed_the_other_version_settings_and_game_installations_", {"removal": removal})} confirmText={tx("ui-daniel-environment-preferences", "uninstall_2")} cancelText={tx("ui-daniel-environment-preferences", "cancel")} isDanger busy={busy} onCancel={closeRemoval} onConfirm={() => void run(removal)}/>}
    <section className="mod-next-step"><div className="mod-step-heading"><span className="mod-component-icon"><MenuIcon name="gamepad"/></span><div><h4>{tx("ui-daniel-environment-preferences", "install_for_the_games_you_choose")}</h4><p>{tx("ui-daniel-environment-preferences", "library_select_a_game_mods_daniel_nr_test")}</p></div></div><p className="mod-step-note">{tx("ui-daniel-environment-preferences", "choose_the_version_and_loading_dll_then_install_for_that_game_preparing_the_environment_")}</p><div className="mod-requirement"><MenuIcon name="info"/><p>{version === "0.6.0" ? (tx("ui-daniel-environment-preferences", "preparation_downloads_verified_dependencies_and_a_private_rocm_runtime_allow_about_3_gb_")) : (tx("ui-daniel-environment-preferences", "version_0_8_0_requires_a_prepared_experimental_package_after_importing_select_it_in_the_"))}</p></div></section>
  </section>;
}

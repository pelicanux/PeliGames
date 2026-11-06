import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/I18nContext";
import { ModalSurface } from "./ModalSurface";
import { ProtonSelector } from "./ProtonSelector";
import { MenuIcon } from "./MenuIcon";
import { openDirectoryPicker } from "../services/tauriService";
import { installLauncher, cancelLauncherInstallation, type LauncherRecipe } from "../services/launcherInstallation";

export function LauncherInstallWizard({ recipe, onClose }: { recipe: LauncherRecipe; onClose: () => void }) {
  const {t: tx} = useI18n();

  const [step, setStep] = useState(0), [directory, setDirectory] = useState(""), [proton, setProton] = useState("");
  const [dependencies, setDependencies] = useState<string[]>([]), [busy, setBusy] = useState(false), [error, setError] = useState(""), [done, setDone] = useState(false), [cancelling, setCancelling] = useState(false);
  const lock = useRef(false);
  const preview = import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window;
  const close = () => { if (!lock.current) onClose(); };
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { event.stopImmediatePropagation(); if (!lock.current) onClose(); } };
    window.addEventListener("keydown", escape, true);
    return () => window.removeEventListener("keydown", escape, true);
  }, [onClose]);
  const chooseDirectory = async () => {
    try { const base = preview ? "/preview/launchers" : await openDirectoryPicker(tx("ui-launcher-install-wizard", "choose_where_to_install_the_launcher")); if (base) setDirectory(`${base.replace(/\/$/, "")}/${recipe.id}`); }
    catch (e) { setError(String(e)); }
  };
  const install = async () => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError("");
    try {
      const result = await installLauncher({ id: recipe.id, directory, proton, dependencies });
      if (result.library_error || !result.registered_count) throw new Error(result.library_error || (tx("ui-launcher-install-wizard", "installed_executable_not_found")));
      setDone(true);
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
    } catch (e) { setError(String(e)); }
    finally { lock.current = false; setBusy(false); setCancelling(false); }
  };
  const cancel = async () => {
    setCancelling(true);
    try { await cancelLauncherInstallation(); } catch (e) { setError(String(e)); setCancelling(false); }
  };
  return <div className="modal-overlay" onClick={event => { if (event.target === event.currentTarget) close(); }}>
    <ModalSurface className="modal-content dialog-glass launcher-install-wizard" role="dialog" aria-modal="true" aria-labelledby="launcher-wizard-title" onDismiss={close} closeDisabled={busy}
      header={<h2 id="launcher-wizard-title"><img src={`/launchers/${recipe.id}.svg`} alt="" />{recipe.name}</h2>}>
      {preview && <p className="nexus-hint">{tx("ui-launcher-install-wizard", "wizard_preview_no_files_will_be_downloaded_or_installed")}</p>}
      {done ? <><h3>{(preview ? tx("ui-launcher-install-wizard", "preview_complete") : tx("ui-launcher-install-wizard", "launcher_added_to_your_library"))}</h3><p>{preview ? tx("ui-launcher-install-wizard", "preview_added") : tx("ui-launcher-install-wizard", "library_added")}</p><button className="btn btn-secondary" onClick={close}>{tx("ui-launcher-install-wizard", "done")}</button></>
        : busy ? <div role="status" className="launcher-install-progress"><MenuIcon name="download" /><h3>{(cancelling ? tx("ui-launcher-install-wizard", "cancelling") : tx("ui-launcher-install-wizard", "installing_launcher"))}</h3><p>{tx("ui-launcher-install-wizard", "peligames_downloads_and_verifies_the_installer_prepares_the_prefix_and_opens_setup_compl")}</p>{!preview && <button className="btn btn-secondary" disabled={cancelling} onClick={() => void cancel()}>{tx("ui-launcher-install-wizard", "cancel_installation")}</button>}</div>
        : <><p className="nexus-hint">{tx("ui-launcher-install-wizard", "step_3", {"step_1": step + 1})}</p>
          {step === 0 && <><h3>{tx("ui-launcher-install-wizard", "prepare_installation")}</h3><p>{tx("ui-launcher-install-wizard", "the_installer_comes_from_the_official_vendor_and_runs_in_a_dedicated_prefix_complete_set")}</p><p className="nexus-hint">{tx("ui-launcher-install-wizard", "these_recipes_are_experimental_launcher_updates_may_require_compatibility_adjustments")}</p></>}
          {step === 1 && <div className="launcher-wizard-fields"><ProtonSelector value={proton} onChange={setProton} label="Proton" /><p className="nexus-hint">{tx("ui-launcher-install-wizard", "recommended_ge_proton_with_umu_choose_an_installed_runner_peligames_prepares_umu")}</p><label>{tx("ui-launcher-install-wizard", "dedicated_installation_folder")}<input className="nexus-launch-options-input" value={directory} onChange={event => setDirectory(event.target.value)} placeholder="/mnt/Jogos/launchers/…" /></label><button className="btn btn-secondary" onClick={() => void chooseDirectory()}><MenuIcon name="folder" />{tx("ui-launcher-install-wizard", "choose_folder")}</button><p className="nexus-hint">{tx("ui-launcher-install-wizard", "choose_a_new_folder_existing_prefixes_and_installations_are_preserved")}</p></div>}
          {step === 2 && <><h3>{tx("ui-launcher-install-wizard", "review_and_install")}</h3><dl className="launcher-wizard-summary"><dt>Launcher</dt><dd>{recipe.name}</dd><dt>Proton</dt><dd>{proton.split("/").pop()}</dd><dt>{tx("ui-launcher-install-wizard", "destination")}</dt><dd>{directory}</dd><dt>{tx("ui-launcher-install-wizard", "source")}</dt><dd>{new URL(recipe.download.url).hostname}</dd></dl>
            {recipe.dependencies.length > 0 && <fieldset><legend>{tx("ui-launcher-install-wizard", "optional_components")}</legend><p className="nexus-hint">{tx("ui-launcher-install-wizard", "install_only_if_needed_components_have_their_own_licenses")}</p>{recipe.dependencies.map(dep => <label className="launcher-wizard-dependency" key={dep.verb}><input type="checkbox" checked={dependencies.includes(dep.verb)} onChange={event => setDependencies(items => event.target.checked ? [...items, dep.verb] : items.filter(item => item !== dep.verb))} /><span>{dep.verb}<small className="nexus-hint">{dep.reason}</small></span></label>)}</fieldset>}
            <p className="nexus-hint">{tx("ui-launcher-install-wizard", "installer_verification")}</p></>}
          <div className="modal-actions launcher-wizard-actions"><button className="btn btn-secondary" onClick={() => step ? setStep(step - 1) : close()}>{(step ? tx("ui-launcher-install-wizard", "back") : tx("ui-launcher-install-wizard", "cancel"))}</button><button className="btn btn-primary" disabled={step === 1 && (!proton || !directory.trim().startsWith("/"))} onClick={() => step === 2 ? void install() : setStep(step + 1)}>{(step === 2 ? tx("ui-launcher-install-wizard", "download_and_install") : tx("ui-launcher-install-wizard", "next"))}</button></div></>}
      {error && <p className="nexus-error" role="alert">{error}</p>}
    </ModalSurface>
  </div>;
}

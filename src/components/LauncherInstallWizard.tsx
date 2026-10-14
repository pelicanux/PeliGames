import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/I18nContext";
import { ModalSurface } from "./ModalSurface";
import { ProtonSelector } from "./ProtonSelector";
import { MenuIcon } from "./MenuIcon";
import { openDirectoryPicker } from "../services/tauriService";
import { installLauncher, cancelLauncherInstallation, type LauncherRecipe } from "../services/launcherInstallation";

export function LauncherInstallWizard({ recipe, onClose }: { recipe: LauncherRecipe; onClose: () => void }) {
  const { language } = useI18n(), pt = language === "pt";
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
    try { const base = preview ? "/preview/launchers" : await openDirectoryPicker(pt ? "Escolha onde instalar o launcher" : "Choose where to install the launcher"); if (base) setDirectory(`${base.replace(/\/$/, "")}/${recipe.id}`); }
    catch (e) { setError(String(e)); }
  };
  const install = async () => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError("");
    try {
      const result = await installLauncher({ id: recipe.id, directory, proton, dependencies });
      if (result.library_error || !result.registered_count) throw new Error(result.library_error || (pt ? "O executável instalado não foi encontrado." : "Installed executable not found."));
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
      {preview && <p className="nexus-hint">{pt ? "Demonstração do assistente. Nenhum arquivo será baixado ou instalado." : "Wizard preview. No files will be downloaded or installed."}</p>}
      {done ? <><h3>{pt ? preview ? "Demonstração concluída" : "Launcher adicionado à biblioteca" : preview ? "Preview complete" : "Launcher added to your library"}</h3><p>{pt ? preview ? "Um exemplo foi adicionado à categoria Launchers somente nesta prévia." : "O executável está na categoria Launchers da biblioteca principal." : "The executable appears under Launchers in the main library after installation."}</p><button className="btn btn-secondary" onClick={close}>{pt ? "Concluir" : "Done"}</button></>
        : busy ? <div role="status" className="launcher-install-progress"><MenuIcon name="download" /><h3>{pt ? cancelling ? "Cancelando…" : "Instalando launcher…" : cancelling ? "Cancelling…" : "Installing launcher…"}</h3><p>{pt ? "O PeliGames baixa e verifica o instalador, prepara o prefixo e abre a instalação. Conclua as etapas na janela do instalador e feche o launcher para finalizar o registro." : "PeliGames downloads and verifies the installer, prepares the prefix and opens setup. Complete setup, then close the launcher to finish registration."}</p>{!preview && <button className="btn btn-secondary" disabled={cancelling} onClick={() => void cancel()}>{pt ? "Cancelar instalação" : "Cancel installation"}</button>}</div>
        : <><p className="nexus-hint">{pt ? `Etapa ${step + 1}/3` : `Step ${step + 1}/3`}</p>
          {step === 0 && <><h3>{pt ? "Preparar a instalação" : "Prepare installation"}</h3><p>{pt ? "O instalador será baixado do fornecedor oficial e executado em um prefixo exclusivo. Você concluirá a instalação na janela do próprio launcher." : "The installer comes from the official vendor and runs in a dedicated prefix. Complete setup in the launcher's own window."}</p><p className="nexus-hint">{pt ? "Estas receitas são experimentais. Atualizações dos launchers podem exigir ajustes de compatibilidade." : "These recipes are experimental. Launcher updates may require compatibility adjustments."}</p></>}
          {step === 1 && <div className="launcher-wizard-fields"><ProtonSelector value={proton} onChange={setProton} label="Proton" /><p className="nexus-hint">{pt ? "Recomendado: GE-Proton com UMU. Escolha uma versão instalada; o UMU será preparado pelo PeliGames." : "Recommended: GE-Proton with UMU. Choose an installed runner; PeliGames prepares UMU."}</p><label>{pt ? "Pasta exclusiva da instalação" : "Dedicated installation folder"}<input className="nexus-launch-options-input" value={directory} onChange={event => setDirectory(event.target.value)} placeholder="/mnt/Jogos/launchers/…" /></label><button className="btn btn-secondary" onClick={() => void chooseDirectory()}><MenuIcon name="folder" />{pt ? "Escolher pasta" : "Choose folder"}</button><p className="nexus-hint">{pt ? "Escolha uma pasta nova. Prefixos e instalações existentes serão preservados." : "Choose a new folder. Existing prefixes and installations are preserved."}</p></div>}
          {step === 2 && <><h3>{pt ? "Revisar e instalar" : "Review and install"}</h3><dl className="launcher-wizard-summary"><dt>Launcher</dt><dd>{recipe.name}</dd><dt>Proton</dt><dd>{proton.split("/").pop()}</dd><dt>{pt ? "Destino" : "Destination"}</dt><dd>{directory}</dd><dt>{pt ? "Origem" : "Source"}</dt><dd>{new URL(recipe.download.url).hostname}</dd></dl>
            {recipe.dependencies.length > 0 && <fieldset><legend>{pt ? "Componentes opcionais" : "Optional components"}</legend><p className="nexus-hint">{pt ? "Instale apenas se necessários. Os componentes têm suas próprias licenças." : "Install only if needed. Components have their own licenses."}</p>{recipe.dependencies.map(dep => <label className="launcher-wizard-dependency" key={dep.verb}><input type="checkbox" checked={dependencies.includes(dep.verb)} onChange={event => setDependencies(items => event.target.checked ? [...items, dep.verb] : items.filter(item => item !== dep.verb))} /><span>{dep.verb}<small className="nexus-hint">{dep.reason}</small></span></label>)}</fieldset>}
            <p className="nexus-hint">{pt ? "A assinatura do instalador será verificada com osslsigncode, que precisa estar instalado no sistema. Os termos do fornecedor serão apresentados na instalação." : "Installer signatures are verified using osslsigncode, which must be installed on your system. Vendor terms appear during setup."}</p></>}
          <div className="modal-actions launcher-wizard-actions"><button className="btn btn-secondary" onClick={() => step ? setStep(step - 1) : close()}>{pt ? step ? "Voltar" : "Cancelar" : step ? "Back" : "Cancel"}</button><button className="btn btn-primary" disabled={step === 1 && (!proton || !directory.trim().startsWith("/"))} onClick={() => step === 2 ? void install() : setStep(step + 1)}>{pt ? step === 2 ? "Baixar e instalar" : "Avançar" : step === 2 ? "Download and install" : "Next"}</button></div></>}
      {error && <p className="nexus-error" role="alert">{error}</p>}
    </ModalSurface>
  </div>;
}

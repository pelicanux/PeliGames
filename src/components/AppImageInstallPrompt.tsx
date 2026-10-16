import { useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { notifyError } from "../services/notifications";
import "./AppImageInstallPrompt.css";
import "./WelcomeModal.css";

/** Remembered "do not ask again" for the loose AppImage install question. */
export const APPIMAGE_PROMPT_KEY = "peligames_appimage_prompt_dismissed";

/** Asked when PeliGames runs from an AppImage file that is not installed:
 * what installing adds, and what keeps working without it. */
export function AppImageInstallPrompt({ target, onClose }: { target: string; onClose: () => void }) {
  const { language } = useI18n(); const pt = language !== "en";
  const [remember, setRemember] = useState(false), [installing, setInstalling] = useState(false), [error, setError] = useState("");
  const close = () => {
    if (installing) return;
    if (remember) { try { localStorage.setItem(APPIMAGE_PROMPT_KEY, "1"); } catch { /* Asked again next time. */ } }
    onClose();
  };
  const install = async () => {
    if (installing) return;
    setInstalling(true); setError("");
    // The installed copy opens and this one closes.
    try { await invoke("install_peligames_appimage"); }
    catch (e) { setError(String(e)); notifyError(pt ? "Instalar PeliGames" : "Install PeliGames", e); setInstalling(false); }
  };
  const title = pt ? "Instalar o PeliGames?" : "Install PeliGames?";
  const installed = pt ? [
    "Botão “Mod Manager Download” do site do Nexus abre direto no PeliGames.",
    "Atalho no menu de aplicativos e o comando peligames no terminal.",
    "Atualizações pelo launcher sem depender de onde o arquivo foi baixado; atalhos de jogos continuam valendo.",
    "Instalar o MangoHUD pelo launcher.",
    "Preparar e formatar discos pedindo a senha uma vez só.",
  ] : [
    "The Nexus site's “Mod Manager Download” button opens straight in PeliGames.",
    "An application menu entry and the peligames terminal command.",
    "Updates through the launcher wherever the file was downloaded; game shortcuts keep working.",
    "Install MangoHUD from the launcher.",
    "Prepare and format drives with a single password prompt.",
  ];
  const loose = pt ? [
    "Mods do Nexus só pelo navegador interno do PeliGames ou importando o arquivo baixado à mão.",
    "Sem atalho no menu de aplicativos.",
    "MangoHUD precisa ser instalado pelo sistema.",
    "Discos: a senha é pedida em cada etapa.",
    "Mover ou apagar o arquivo AppImage quebra os atalhos de jogos.",
  ] : [
    "Nexus mods only through PeliGames' internal browser or by importing a manual download.",
    "No application menu entry.",
    "MangoHUD must be installed through the system.",
    "Drives: the password is asked at every step.",
    "Moving or deleting the AppImage file breaks game shortcuts.",
  ];
  return createPortal(<div className="modal-overlay confirmation-overlay" style={{ zIndex: 9998 }}>
    <ModalSurface header={<h2><MenuIcon name="download" />{title}</h2>} onDismiss={close} closeDisabled={installing} role="dialog" aria-modal="true" aria-label={title} className="modal-content dialog-glass appimage-prompt welcome-tour-modern">
      <div className="modal-body">
        <p className="appimage-prompt-intro">{pt ? "Você está usando o PeliGames direto do arquivo AppImage. Ele funciona assim, mas instalar libera alguns recursos. A instalação fica na sua pasta pessoal, não pede senha e o arquivo AppImage baixado é removido." : "You are running PeliGames straight from the AppImage file. It works this way, but installing unlocks a few features. It installs into your home folder, needs no password, and the downloaded AppImage file is removed."}</p>
        <div className="appimage-prompt-columns">
          <section><h3>{pt ? "Instalando" : "Installed"}</h3><ul>{installed.map(item => <li key={item}>{item}</li>)}</ul></section>
          <section className="appimage-prompt-limits"><h3>{pt ? "Só com o AppImage" : "AppImage only"}</h3><ul>{loose.map(item => <li key={item}>{item}</li>)}</ul></section>
        </div>
        <p className="appimage-prompt-target">{pt ? "Destino:" : "Destination:"} <code>{target}</code></p>
        <label className="appimage-prompt-remember"><input type="checkbox" checked={remember} disabled={installing} onChange={event => setRemember(event.target.checked)} />{pt ? "Não perguntar de novo (dá para instalar depois nas Configurações)" : "Don't ask again (you can install later in Settings)"}</label>
        {error && <p role="alert" className="preferences-error">{error}</p>}
      </div>
      <div className="modal-actions">
        <button type="button" className="btn btn-secondary" disabled={installing} onClick={close}>{pt ? "Continuar sem instalar" : "Continue without installing"}</button>
        <button type="button" className="btn btn-primary" disabled={installing} onClick={() => void install()}>{installing ? (pt ? "Instalando…" : "Installing…") : (pt ? "Instalar agora" : "Install now")}</button>
      </div>
    </ModalSurface>
  </div>, document.body);
}

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
  const {t: tx} = useI18n();

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
    catch (e) { setError(String(e)); notifyError(tx("ui-app-image-install-prompt", "install_peligames"), e); setInstalling(false); }
  };
  const title = tx("ui-app-image-install-prompt", "install_peligames_2");
  const installed = [tx("ui-app-image-install-prompt", "the_nexus_site_s_mod_manager_download_button_opens_straight_in_peligames"), tx("ui-app-image-install-prompt", "an_application_menu_entry_and_the_peligames_terminal_command"), tx("ui-app-image-install-prompt", "updates_through_the_launcher_wherever_the_file_was_downloaded_game_shortcuts_keep_workin"), tx("ui-app-image-install-prompt", "install_mangohud_from_the_launcher"), tx("ui-app-image-install-prompt", "prepare_and_format_drives_with_a_single_password_prompt")];
  const loose = [tx("ui-app-image-install-prompt", "nexus_mods_only_through_peligames_internal_browser_or_by_importing_a_manual_download"), tx("ui-app-image-install-prompt", "no_application_menu_entry"), tx("ui-app-image-install-prompt", "mangohud_must_be_installed_through_the_system"), tx("ui-app-image-install-prompt", "drives_the_password_is_asked_at_every_step"), tx("ui-app-image-install-prompt", "moving_or_deleting_the_appimage_file_breaks_game_shortcuts")];
  return createPortal(<div className="modal-overlay confirmation-overlay" style={{ zIndex: 9998 }}>
    <ModalSurface header={<h2><MenuIcon name="download" />{title}</h2>} onDismiss={close} closeDisabled={installing} role="dialog" aria-modal="true" aria-label={title} className="modal-content dialog-glass appimage-prompt welcome-tour-modern">
      <div className="modal-body">
        <p className="appimage-prompt-intro">{tx("ui-app-image-install-prompt", "you_are_running_peligames_straight_from_the_appimage_file_it_works_this_way_but_installi")}</p>
        <div className="appimage-prompt-columns">
          <section><h3>{tx("ui-app-image-install-prompt", "installed")}</h3><ul>{installed.map(item => <li key={item}>{item}</li>)}</ul></section>
          <section className="appimage-prompt-limits"><h3>{tx("ui-app-image-install-prompt", "appimage_only")}</h3><ul>{loose.map(item => <li key={item}>{item}</li>)}</ul></section>
        </div>
        <p className="appimage-prompt-target">{tx("ui-app-image-install-prompt", "destination")} <code>{target}</code></p>
        <label className="appimage-prompt-remember"><input type="checkbox" checked={remember} disabled={installing} onChange={event => setRemember(event.target.checked)} />{tx("ui-app-image-install-prompt", "don_t_ask_again_you_can_install_later_in_settings")}</label>
        {error && <p role="alert" className="preferences-error">{error}</p>}
      </div>
      <div className="modal-actions">
        <button type="button" className="btn btn-secondary" disabled={installing} onClick={close}>{tx("ui-app-image-install-prompt", "continue_without_installing")}</button>
        <button type="button" className="btn btn-primary" disabled={installing} onClick={() => void install()}>{installing ? (tx("ui-app-image-install-prompt", "installing")) : (tx("ui-app-image-install-prompt", "install_now"))}</button>
      </div>
    </ModalSurface>
  </div>, document.body);
}

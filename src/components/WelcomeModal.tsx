import { ModalSurface } from "./ModalSurface";
import { useI18n } from "../i18n/I18nContext";

/** Launcher onboarding is independent of any mod's hardware/setup requirements. */
export function WelcomeModal({ onStart }: { onStart: () => void }) {
  const { t } = useI18n();
  return <div className="modal-overlay">
    <ModalSurface hideClose className="modal-content dialog-glass welcome-modal" role="dialog" aria-modal="true" aria-labelledby="launcher-welcome-title">
      <h2 id="launcher-welcome-title">{t("welcome", "title")}</h2>
      <button type="button" className="primary" autoFocus onClick={onStart}>{t("welcome", "start")}</button>
    </ModalSurface>
  </div>;
}

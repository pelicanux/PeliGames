import { useI18n } from "../i18n/I18nContext";

export function CloseButton({ onClose, disabled = false }: { onClose?: () => void; disabled?: boolean }) {
  const { language } = useI18n();
  const label = language === "en" ? "Close" : "Fechar";
  return <button type="button" className="btn-titlebar window-control close surface-close-button"
    aria-label={label} title={label} disabled={disabled || !onClose}
    onClick={event => { event.stopPropagation(); onClose?.(); }}>
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="m18 6-12 12M6 6l12 12" /></svg>
  </button>;
}

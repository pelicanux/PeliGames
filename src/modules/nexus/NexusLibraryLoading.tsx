import { useI18n } from "../../i18n/I18nContext";
export function NexusLibraryLoading({ reducedMotion = false }: { reducedMotion?: boolean }) {
  const { t } = useI18n();
  return <div className={`nexus-library-loading${reducedMotion ? " reduced-motion" : ""}`} role="status" aria-live="polite">
    <div className="nexus-loading-emblem" aria-hidden="true"><span className="nexus-loading-ring" /><img src="/peligames.svg" alt="" /></div>
    <p>{t("app", "scanningGames")}</p>
  </div>;
}

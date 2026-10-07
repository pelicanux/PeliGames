import { useI18n } from "../i18n/I18nContext";
import { ACCENT_THEMES } from "../theme/accentTheme";
import { useAccentTheme } from "../theme/ThemeProvider";

export function AccentColorPreference() {
  const { t } = useI18n();
  const { accent, setAccent, appearance, setAppearance } = useAccentTheme();
  return <section className="accent-preference" aria-labelledby="appearance-heading">
    <h3 id="appearance-heading">{t("theme", "appearance")}</h3>
    <div className="appearance-controls">
      <button type="button" className="appearance-toggle"
        aria-label={`${t("theme", "appearance")}: ${t("theme", appearance)}`}
        title={t("theme", appearance === "dark" ? "light" : "dark")}
        onClick={() => setAppearance(appearance === "dark" ? "light" : "dark")}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          {appearance === "dark" ? <path d="M20.9 13a9 9 0 0 1-9.9-9.9A9 9 0 1 0 20.9 13Z" /> : <><circle cx="12" cy="12" r="4" /><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5" /></>}
        </svg>
        <span>{t("theme", appearance)}</span>
      </button>
      <div className="accent-color-options" role="group" aria-label={t("theme", "title")}>
        {ACCENT_THEMES.map(theme => <button key={theme.id} type="button"
          aria-label={t("theme", theme.id)} title={t("theme", theme.id)}
          aria-pressed={accent === theme.id} onClick={() => setAccent(theme.id)}>
          <span className="accent-color-swatch" style={{ background: theme.color }} aria-hidden="true">
            {accent === theme.id && <svg viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3"><path d="m5 12 4 4L19 6" /></svg>}
          </span>
        </button>)}
      </div>
    </div>
  </section>;
}

import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/I18nContext";
import { MAX_INTERFACE_SCALE, useInterfaceScale } from "../hooks/useInterfaceScale";
import { MenuIcon } from "./MenuIcon";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  disabled: boolean;
  preferencesOpen: boolean;
  onPreferences: () => void;
  performanceMode: boolean;
  onPerformanceModeChange: (enabled: boolean) => void;
}

export function LauncherSettingsMenu({ open: showSettings, onOpenChange: setShowSettings, disabled, preferencesOpen, onPreferences, performanceMode, onPerformanceModeChange: setPerformanceMode }: Props) {
  const { t, language, setLanguage } = useI18n();
  const [showLangDropdown, setShowLangDropdown] = useState(false);
  const [showEffectsDropdown, setShowEffectsDropdown] = useState(false);
  const settingsRef = useRef<HTMLDivElement>(null);
  const [uiScale, setUiScale] = useInterfaceScale();
  useEffect(() => {
    if (!showSettings) { setShowEffectsDropdown(false); setShowLangDropdown(false); }
  }, [showSettings]);
  useEffect(() => {
    if (!showSettings) return;
    const closeOutside = (event: MouseEvent) => {
      if (!settingsRef.current?.contains(event.target as Node)) setShowSettings(false);
    };
    document.addEventListener("mousedown", closeOutside);
    return () => document.removeEventListener("mousedown", closeOutside);
  }, [showSettings, setShowSettings]);
  return (
    <div style={{ position: "relative" }} ref={settingsRef}>
      <button
        data-tauri-drag-region="false"
        aria-pressed={showSettings || preferencesOpen}
        aria-haspopup="dialog" aria-expanded={showSettings} aria-controls="launcher-settings-menu"
        className="btn-titlebar"
        style={{ padding: "0.3rem 0.5rem", borderRadius: "4px", display: "flex", alignItems: "center", justifyContent: "center", background: showSettings ? "rgba(var(--surface-255-255-255, 255, 255, 255), 0.15)" : "transparent", border: "none", color: "var(--tone-94a3b8, #94a3b8)", cursor: "pointer" }}
        disabled={disabled}
        onClick={() => { if (!disabled) setShowSettings(!showSettings); }}
        title={t("settings", "title")}
      >
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
      </button>

      {showSettings && (
        <div id="launcher-settings-menu" className="dialog-popover launcher-settings-menu" role="dialog" aria-label={t("settings", "title")}>
          <div className="launcher-settings-items">
            <button
              className="launcher-settings-preferences"
              onClick={() => { onPreferences(); setShowSettings(false); }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
                <span>{t("settings", "preferences")}</span>
              </div>
            </button>
            <div className="effects-preference">
              <span className="effects-preference-label">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20"/></svg>
                {t("settings", "language")}
              </span>
              <div className="effects-menu-anchor">
                <button type="button" className="effects-menu-trigger" aria-label={t("settings", "language")}
                  aria-haspopup="menu" aria-expanded={showLangDropdown}
                  onClick={e => { e.stopPropagation(); setShowLangDropdown(!showLangDropdown); setShowEffectsDropdown(false); }}>
                  {language === "pt" ? "BR" : "EN"}
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="m6 9 6 6 6-6"/></svg>
                </button>
                {showLangDropdown && <div className="effects-menu" role="menu" onKeyDown={e => { if (e.key === "Escape") setShowLangDropdown(false); }}>
                  {(["pt", "en"] as const).map(lang => <button key={lang} type="button" role="menuitemradio" aria-checked={language === lang}
                    onClick={() => { setLanguage(lang); setShowLangDropdown(false); setShowSettings(false); }}>
                    {lang === "pt" ? "BR" : "EN"}
                  </button>)}
                </div>}
              </div>
            </div>

            <div className="effects-preference">
              <span className="effects-preference-label"><MenuIcon name="bolt" /> {t("settings", "effects")}</span>
              <div className="effects-menu-anchor">
                <button type="button" className="effects-menu-trigger" aria-label={t("settings", "effects")}
                  aria-haspopup="menu" aria-expanded={showEffectsDropdown} aria-controls="interface-effects-menu"
                  onClick={() => { setShowEffectsDropdown(!showEffectsDropdown); setShowLangDropdown(false); }}
                  onKeyDown={e => { if (e.key === "Escape") setShowEffectsDropdown(false); }}>
                  {t("settings", performanceMode ? "performance" : "elegant")}
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="m6 9 6 6 6-6"/></svg>
                </button>
                {showEffectsDropdown && <div id="interface-effects-menu" className="effects-menu" role="menu"
                  onKeyDown={e => { if (e.key === "Escape") setShowEffectsDropdown(false); }}>
                  <button type="button" role="menuitemradio" aria-checked={!performanceMode} onClick={() => { setPerformanceMode(false); setShowEffectsDropdown(false); }}>{t("settings", "elegant")}</button>
                  <button type="button" role="menuitemradio" aria-checked={performanceMode} onClick={() => { setPerformanceMode(true); setShowEffectsDropdown(false); }}>{t("settings", "performance")}</button>
                </div>}
              </div>
            </div>
            <div className="launcher-settings-scale">
              <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/><line x1="11" x2="11" y1="8" y2="14"/><line x1="8" x2="14" y1="11" y2="11"/></svg>
                <span>{t("settings", "scale")}</span>
              </div>
              <div className="launcher-settings-scale-controls">
                <button className="launcher-settings-scale-button" onClick={() => setUiScale(Math.max(0.6, uiScale - 0.1))}>-</button>
                <span style={{ fontSize: "0.9rem", color: "var(--accent-light)" }}>{Math.round(uiScale * 100)}%</span>
                <button className="launcher-settings-scale-button" disabled={uiScale >= MAX_INTERFACE_SCALE} onClick={() => setUiScale(Math.min(MAX_INTERFACE_SCALE, Math.round((uiScale + 0.1) * 10) / 10))}>+</button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

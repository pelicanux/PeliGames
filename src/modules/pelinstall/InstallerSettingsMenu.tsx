import { useId, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useBoundedDropdown } from "../../hooks/useBoundedDropdown";
import { MenuIcon } from "../../components/MenuIcon";
import {useLanguageChoices} from "../../i18n/useLanguageChoices";
import { MAX_INTERFACE_SCALE, useInterfaceScale } from "../../hooks/useInterfaceScale";

export function InstallerSettingsMenu() {

  const {preference, options, label: languageLabel, loading, error, setLanguage, t} = useLanguageChoices();
  const [scale, setScale] = useInterfaceScale();
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<CSSProperties>({});
  const trigger = useRef<HTMLButtonElement>(null), menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const [languageOpen, setLanguageOpen] = useState(false);
  const languageAnchor = useRef<HTMLDivElement>(null), languageMenu = useRef<HTMLDivElement>(null);
  const languagePosition = useBoundedDropdown(languageOpen, languageAnchor, languageMenu, setLanguageOpen);
  const closeLanguage = () => { setLanguageOpen(false); languageAnchor.current?.querySelector<HTMLButtonElement>("button")?.focus(); };
  const label = t("ui-installer-settings-menu", "installer_settings");
  useLayoutEffect(() => {
    if (!open) return;
    const update = () => {
      const rect = trigger.current?.getBoundingClientRect();
      if (!rect) return;
      const margin = 8;
      const width = Math.max(0, Math.min(260 * scale, window.innerWidth - margin * 2));
      const maxHeight = Math.max(0, window.innerHeight - margin * 2);
      const height = Math.min(menu.current?.offsetHeight ?? 0, maxHeight);
      // Keep the recovery controls inside the window, even after scaling or resizing.
      setPosition({
        left: Math.max(margin, Math.min(rect.left, window.innerWidth - width - margin)),
        top: Math.max(margin, Math.min(rect.bottom + margin, window.innerHeight - height - margin)),
        width, maxHeight,
      });
    };
    const outside = (event: PointerEvent) => { if (!trigger.current?.contains(event.target as Node) && !menu.current?.contains(event.target as Node) && !languageMenu.current?.contains(event.target as Node)) { setLanguageOpen(false); setOpen(false); } };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { event.stopImmediatePropagation(); if (languageOpen) closeLanguage(); else { setOpen(false); trigger.current?.focus(); } } };
    update();
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape, true);
    window.addEventListener("resize", update);
    const observer = new ResizeObserver(update);
    if (trigger.current) observer.observe(trigger.current);
    if (menu.current) observer.observe(menu.current);
    return () => { observer.disconnect(); document.removeEventListener("pointerdown", outside); document.removeEventListener("keydown", escape, true); window.removeEventListener("resize", update); };
  }, [open, scale, languageOpen]);
  return <>
    <button ref={trigger} type="button" className="btn-titlebar pelinstall-settings-button" aria-label={label} title={label}
      aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => { setLanguageOpen(false); setOpen(!open); }}><MenuIcon name="settings" /></button>
    {open && createPortal(<div ref={menu} id={id} role="dialog" aria-label={label} className="dialog-popover pelinstall-settings-menu" style={position}>
      <div className="effects-preference">
        <span className="effects-preference-label"><MenuIcon name="language" />{t("settings", "language")}</span>
        <div ref={languageAnchor} className="custom-select-wrapper">
          <button type="button" className={`custom-select-trigger pelinstall-language-trigger ${languageOpen ? "open" : ""}`}
            disabled={loading} aria-busy={loading} aria-label={t("settings", "language")} aria-haspopup="listbox" aria-expanded={languageOpen} aria-controls={languageOpen ? `${id}-language` : undefined}
            onClick={() => setLanguageOpen(!languageOpen)} onKeyDown={event => { if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); setLanguageOpen(true); } }}>
            <span>{languageLabel}</span><span className="select-arrow" aria-hidden="true">▼</span>
          </button>
        </div>
      </div>
      {error && <p role="alert">{t("localization", "loadError")}</p>}
      <div className="pelinstall-scale-preference">
        <span>{t("settings", "scale")}</span>
        <div><button type="button" disabled={scale <= 0.6} aria-label={t("ui-installer-settings-menu", "decrease_scale")} onClick={() => setScale(value => Math.max(0.6, Math.round((value - 0.1) * 10) / 10))}>−</button>
          <output aria-live="polite">{Math.round(scale * 100)}%</output>
          <button type="button" disabled={scale >= MAX_INTERFACE_SCALE} aria-label={t("ui-installer-settings-menu", "increase_scale")} onClick={() => setScale(value => Math.min(MAX_INTERFACE_SCALE, Math.round((value + 0.1) * 10) / 10))}>+</button></div>
      </div>
    </div>, document.body)}
    {open && languageOpen && createPortal(<div ref={languageMenu} id={`${id}-language`} role="listbox" aria-label={t("settings", "language")}
      className="custom-select-menu pelinstall-entry-menu pelinstall-language-menu" style={{ ...languagePosition, zIndex: 1201 }}
      onKeyDown={event => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          const buttons = Array.from(languageMenu.current?.querySelectorAll<HTMLButtonElement>("button") || []);
          const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
          buttons[(index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
        }
      }}>
      {options.map(option => <button key={option.value} type="button" role="option" aria-selected={preference === option.value}
        autoFocus={preference === option.value} className={`custom-select-option ${preference === option.value ? "selected" : ""}`}
        onClick={() => { void setLanguage(option.value); closeLanguage(); }}>{option.label}</button>)}
    </div>, document.body)}
  </>;
}

import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useBoundedDropdown } from "../hooks/useBoundedDropdown";
import { useI18n } from "../i18n/I18nContext";
import { HoverTooltip } from "./HoverTooltip";

export interface InstalledProton { name: string; path: string; }

export function ProtonSelector({ value, onChange, label, disabled = false }: { value: string; onChange: (value: string) => void; label?: string; disabled?: boolean }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [installed, setInstalled] = useState<InstalledProton[]>([]);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const anchor = useRef<HTMLDivElement>(null), menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const position = useBoundedDropdown(open, anchor, menu, setOpen, 260);
  useEffect(() => {
    let active = true;
    let request = 0;
    const scan = () => {
      const current = ++request;
      setStatus("loading");
      invoke<InstalledProton[]>("scan_installed_protons").then(result => {
        if (active && current === request) { setInstalled(result); setStatus("ready"); }
      }).catch(() => { if (active && current === request) setStatus("error"); });
    };
    scan();
    window.addEventListener("protonRunnersChanged", scan);
    return () => { active = false; window.removeEventListener("protonRunnersChanged", scan); };
  }, []);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);
  const options = installed;
  const selected = options.find(option => option.path === value);
  return <div className="installation-field">
    <div className="installation-type-label"><span className="installation-field-label">{label ?? t("gameModes", "installerType")}</span>
      <HoverTooltip text={t("gameModes", "protonHint")}>
        <button type="button" className="installation-type-info" aria-label={t("gameModes", "protonInfo")}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
            <circle cx="12" cy="12" r="9" /><path d="M12 11v6M12 7h.01" />
          </svg>
        </button>
      </HoverTooltip>
    </div>
    <div ref={anchor}>
      <button type="button" className="custom-select-trigger proton-trigger" aria-haspopup="listbox" aria-expanded={open}
        disabled={disabled} aria-label={label ?? t("gameModes", "installerType")} aria-controls={open ? id : undefined} onClick={() => setOpen(value => !value)}>
        <span>{selected?.name ?? (value ? value.split(/[\\/]/).pop() : t("gameModes", "selectProton"))}</span><span className="select-arrow">▼</span>
      </button>
      {open && createPortal(<div ref={menu} id={id} role="listbox" aria-label={label ?? t("gameModes", "installerType")}
        className="custom-select-menu route-select-menu proton-options select-scroll-frame" style={position}
        onKeyDown={event => {
          if (event.key === "Escape") { setOpen(false); anchor.current?.querySelector<HTMLButtonElement>("button")?.focus(); }
          if (event.key === "Tab") setOpen(false);
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button[role=option]") ?? []);
            const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
            buttons[(index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
          }
        }}>
        <div className="select-options-scroll">
        {options.map((option, index) => <button key={option.path} type="button" role="option" aria-selected={value === option.path}
          className={`custom-select-option ${value === option.path ? "selected" : ""}`} autoFocus={index === Math.max(0, options.findIndex(option => option.path === value))}
          onClick={() => { onChange(option.path); setOpen(false); anchor.current?.querySelector<HTMLButtonElement>("button")?.focus(); }}>
          <span>{option.name}</span>
        </button>)}
        {(status !== "ready" || !installed.length) && <p className="proton-scan-status">{t("gameModes", status === "loading" ? "scanningProtons" : status === "error" ? "protonScanError" : installed.length ? "installedProtons" : "noInstalledProtons")}</p>}
        <p className="proton-scan-status">Versões instaladas da Steam. <button type="button" className="proton-preferences-link" onClick={() => { setOpen(false); window.dispatchEvent(new Event("openProtonPreferences")); }}>Clique aqui</button></p>
        </div>
      </div>, document.body)}
    </div>
  </div>;
}

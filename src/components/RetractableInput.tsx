import { useId, useRef, type Dispatch, type SetStateAction, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useBoundedDropdown } from "../hooks/useBoundedDropdown";
import { useI18n } from "../i18n/I18nContext";

/** A small editor anchored to its trigger, outside clipped game panels. */
export function RetractableInput({ open, setOpen, label, value, onChange, onSubmit, onOpen, disabled, error, maxLength = 4096, children }: {
  open: boolean; setOpen: Dispatch<SetStateAction<boolean>>; label: string; value: string;
  onChange: (value: string) => void; onSubmit: () => void; onOpen?: () => void;
  disabled?: boolean; error?: string; maxLength?: number; children?: ReactNode;
}) {
  const { language } = useI18n(), pt = language === "pt";
  const anchor = useRef<HTMLDivElement>(null), menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const position = useBoundedDropdown(open, anchor, menu, setOpen, 300, "right");
  return <div ref={anchor} className="retractable-input-anchor">
    <button type="button" className="modern-mini-btn" data-dropdown-trigger aria-label={`${pt ? "Editar" : "Edit"} ${label}`} aria-expanded={open} aria-controls={open ? id : undefined} disabled={disabled} onClick={() => { if (!open) onOpen?.(); setOpen(!open); }}>{pt ? "Editar" : "Edit"}</button>
    {open && createPortal(<div ref={menu} id={id} className="modern-retractable-input" style={position} onKeyDown={event => {
      if (event.key === "Escape") { setOpen(false); anchor.current?.querySelector("button")?.focus(); }
    }}><form onSubmit={event => { event.preventDefault(); if (!disabled) onSubmit(); }}>
      <input autoFocus aria-label={label} value={value} maxLength={maxLength} disabled={disabled} onChange={event => onChange(event.target.value)} onKeyDown={event => { if (event.key === "Enter" && event.nativeEvent.isComposing) event.preventDefault(); }} />
      <div className="retractable-input-footer"><small>{pt ? "Enter para salvar" : "Enter to save"}</small>{children}</div>
      {error && <p role="alert">{error}</p>}
    </form></div>, document.body)}
  </div>;
}

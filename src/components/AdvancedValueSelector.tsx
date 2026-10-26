import { useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useBoundedDropdown } from "../hooks/useBoundedDropdown";

export function AdvancedValueSelector({ label, value, options, onChange, disabled }: {
  label: string; value: string; options: { value: string; label: string }[];
  onChange: (value: string) => void; disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null), menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const position = useBoundedDropdown(open, anchor, menu, setOpen, 180);
  const restoreFocus = () => anchor.current?.querySelector<HTMLButtonElement>("button")?.focus();
  return <div ref={anchor} className="custom-select-wrapper advanced-value-selector">
    <button type="button" className={`custom-select-trigger ${open ? "open" : ""}`} disabled={disabled}
      aria-label={label} aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? id : undefined}
      onClick={() => setOpen(current => !current)} onKeyDown={event => {
        if (["ArrowDown", "ArrowUp"].includes(event.key)) { event.preventDefault(); setOpen(true); }
      }}>
      <span>{options.find(option => option.value === value)?.label}</span><span className="select-arrow" aria-hidden="true">▾</span>
    </button>
    {open && createPortal(<div ref={menu} id={id} role="listbox" aria-label={label}
      className="custom-select-menu route-select-menu modern-proton-menu" style={position} onKeyDown={event => {
        if (event.key === "Escape") { event.stopPropagation(); setOpen(false); restoreFocus(); }
        const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button") || []);
        const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
        if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
          event.preventDefault();
          buttons[event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();
        }
        if (event.key === "Tab") setOpen(false);
      }}>
      {options.map(option => <button key={option.value} type="button" role="option" aria-selected={value === option.value}
        autoFocus={value === option.value} className={`custom-select-option ${value === option.value ? "selected" : ""}`}
        onClick={() => { onChange(option.value); setOpen(false); restoreFocus(); }}>{option.label}</button>)}
    </div>, document.body)}
  </div>;
}

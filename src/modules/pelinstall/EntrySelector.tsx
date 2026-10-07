import { useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useBoundedDropdown } from "../../hooks/useBoundedDropdown";

export function EntrySelector({ label, value, options, onChange, disabled = false }: {
  label: string; value: string; options: { value: string; label: string }[];
  onChange: (value: string) => void; disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const position = useBoundedDropdown(open, anchor, menu, setOpen);
  const select = (next: string) => { onChange(next); setOpen(false); anchor.current?.querySelector<HTMLButtonElement>("button")?.focus(); };
  return <div ref={anchor} className="custom-select-wrapper pelinstall-entry-selector">
    <button type="button" className={`custom-select-trigger ${open ? "open" : ""}`} disabled={disabled}
      aria-label={label} aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? id : undefined}
      onClick={() => setOpen(!open)} onKeyDown={event => { if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); setOpen(true); } }}>
      <span>{options.find(option => option.value === value)?.label || "Selecionar executável"}</span><span className="select-arrow" aria-hidden="true">▼</span>
    </button>
    {open && createPortal(<div ref={menu} id={id} role="listbox" aria-label={label} className="custom-select-menu pelinstall-entry-menu" style={position}
      onKeyDown={event => { const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button") || []); const index = buttons.indexOf(document.activeElement as HTMLButtonElement); if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); buttons[(index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus(); } }}>
      {options.map(option => <button key={option.value} type="button" role="option" aria-selected={value === option.value}
        className={`custom-select-option ${value === option.value ? "selected" : ""}`} onClick={() => select(option.value)}>{option.label}</button>)}
    </div>, document.body)}
  </div>;
}

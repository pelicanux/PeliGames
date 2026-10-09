import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { MenuIcon } from "../../components/MenuIcon";
import { useBoundedDropdown } from "../../hooks/useBoundedDropdown";
import { useNexusText } from "./text";

export function NexusPageSizeSelector({ value, disabled, onChange }: { value: number; disabled: boolean; onChange: (value: number) => void }) {
  const text = useNexusText();
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null), menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const position = useBoundedDropdown(open, anchor, menu, setOpen);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);
  const restoreFocus = () => anchor.current?.querySelector<HTMLButtonElement>("button")?.focus();
  return <div ref={anchor} className="nexus-catalog-page-size">
    <button type="button" className={`custom-select-trigger proton-trigger nexus-compact-select ${open ? "open" : ""}`} aria-label={text.modsPerPage} aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? id : undefined} disabled={disabled} onClick={() => setOpen(current => !current)}>
      <MenuIcon name="eye" /><span>{value} Mods</span><span className="select-arrow"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg></span>
    </button>
    {open && createPortal(<div ref={menu} id={id} role="listbox" aria-label={text.modsPerPage} className="custom-select-menu route-select-menu nexus-compact-select-menu" style={{ ...position, zIndex: Number((anchor.current?.closest<HTMLElement>(".nexus-popup-overlay")?.style.zIndex) || 1200) + 1 }} onKeyDown={event => {
      if (event.key === "Escape") { setOpen(false); restoreFocus(); }
      if (event.key === "Tab") setOpen(false);
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const options = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>('button[role="option"]') ?? []);
        const index = options.indexOf(document.activeElement as HTMLButtonElement);
        options[(index + (event.key === "ArrowDown" ? 1 : -1) + options.length) % options.length]?.focus();
      }
    }}>{[20, 30, 40, 50].map(size => <button key={size} type="button" role="option" aria-selected={value === size} autoFocus={value === size} className={`custom-select-option ${value === size ? "selected" : ""}`} onClick={() => { setOpen(false); restoreFocus(); onChange(size); }}>{size} Mods</button>)}</div>, document.body)}
  </div>;
}

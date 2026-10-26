import { useEffect, useLayoutEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useBoundedDropdown } from "../hooks/useBoundedDropdown";

// Shared choice menu, using the language/performance selector material.
export function SelectionMenu({ label, value, options, onChange, disabled = false, className = "", menuClassName = "", leadingIcon, minMenuWidth = 0, onOpen }: {
  label: string; value: string; options: { value: string; label: string; disabled?: boolean }[];
  onChange: (value: string) => void; disabled?: boolean; className?: string;
  leadingIcon?: ReactNode;
  minMenuWidth?: number;
  onOpen?: () => void;
  /** Extra class of the option list, which opens in document.body. */
  menuClassName?: string;
}) {
  const [open, setOpen] = useState(false);
  const [fieldsetDisabled, setFieldsetDisabled] = useState(false);
  const anchor = useRef<HTMLDivElement>(null), menu = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const id = useId();
  const position = useBoundedDropdown(open, anchor, menu, setOpen, minMenuWidth);
  const blocked = disabled || fieldsetDisabled;
  useEffect(() => {
    const fieldsets: HTMLFieldSetElement[] = [];
    let parent = anchor.current?.parentElement;
    while (parent) { if (parent instanceof HTMLFieldSetElement) fieldsets.push(parent); parent = parent.parentElement; }
    const update = () => setFieldsetDisabled(fieldsets.some(fieldset => fieldset.disabled));
    update();
    const observer = new MutationObserver(update);
    fieldsets.forEach(fieldset => observer.observe(fieldset, { attributes: true, attributeFilter: ["disabled"] }));
    return () => observer.disconnect();
  }, []);
  useEffect(() => { if (blocked) setOpen(false); }, [blocked]);
  const focusOption = (button?: HTMLButtonElement) => {
    if (!button) return;
    button.focus({preventScroll:true});
    const viewport=button.parentElement;
    if (viewport) {
      const top=button.offsetTop, bottom=top+button.offsetHeight;
      if (top<viewport.scrollTop) viewport.scrollTop=top;
      else if (bottom>viewport.scrollTop+viewport.clientHeight) viewport.scrollTop=bottom-viewport.clientHeight;
    }
  };
  useLayoutEffect(()=>{
    if (!open||blocked) return;
    const selected=menu.current?.querySelector<HTMLButtonElement>('button[role="option"][aria-selected="true"]:not(:disabled)')
      ?? menu.current?.querySelector<HTMLButtonElement>('button[role="option"]:not(:disabled)');
    focusOption(selected??undefined);
  },[open,blocked]);
  const close = () => { setOpen(false); trigger.current?.focus({preventScroll:true}); };
  // The menu lives in document.body: it must open above whatever window holds
  // the selector (the Matrix chat sits at 2100, Nexus popups set their own).
  const overlay = anchor.current?.closest<HTMLElement>(".nexus-popup-overlay, .modal-overlay, .capture-overlay-preview-backdrop");
  const popupLayer = Math.max(1200, Number(overlay?.style.zIndex) || 0, overlay ? Number(getComputedStyle(overlay).zIndex) || 0 : 0);
  return <div ref={anchor} className={`custom-select-wrapper selection-menu ${className}`}>
    <button ref={trigger} type="button" className={`custom-select-trigger ${open ? "open" : ""}`} disabled={blocked}
      aria-label={label} aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? id : undefined}
      onClick={() => { if (!open) onOpen?.(); setOpen(!open); }} onKeyDown={event => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); if (!open) onOpen?.(); setOpen(true); }
      }}>
      {leadingIcon}
      <span>{options.find(option => option.value === value)?.label ?? value}</span>
      <span className="select-arrow" aria-hidden="true"><svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="m6 9 6 6 6-6" /></svg></span>
    </button>
    {open && !blocked && createPortal(<div ref={menu} id={id} role="listbox" aria-label={label} className={`custom-select-menu selection-options ${menuClassName}`.trim()} style={{ ...position, zIndex: popupLayer + 1 }}
      onKeyDown={event => {
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(); }
        if (event.key === "Tab") { event.preventDefault(); close(); }
        const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>('button[role="option"]:not(:disabled)') ?? []);
        if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
          event.preventDefault();
          const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
          const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1 : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length;
          focusOption(buttons[next]);
        }
      }}>
      <div className="selection-options-scroll">
      {options.map(option => <button key={option.value} type="button" role="option" aria-selected={value === option.value} disabled={option.disabled}
        className={`custom-select-option ${value === option.value ? "selected" : ""}`} onClick={() => { if (!blocked) { close(); onChange(option.value); } }}>{option.label}</button>)}
      </div>
    </div>, document.body)}
  </div>;
}

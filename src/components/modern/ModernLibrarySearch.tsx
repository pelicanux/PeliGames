import {useEffect, useId, useRef, useState} from "react";
import {createPortal} from "react-dom";
import {useBoundedDropdown} from "../../hooks/useBoundedDropdown";
import {useI18n} from "../../i18n/I18nContext";
import {MenuIcon} from "../MenuIcon";
import "./ModernLibrarySearch.css";

/** The search keeps its toolbar slot; a narrow slot exposes an anchored editor. */
export function ModernLibrarySearch({value, onChange}: {value:string; onChange:(value:string)=>void}) {
  const {t} = useI18n();
  const [compact, setCompact] = useState(false), [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null), panel = useRef<HTMLDivElement>(null), input = useRef<HTMLInputElement>(null), trigger = useRef<HTMLButtonElement>(null);
  const id = useId();
  const position = useBoundedDropdown(open && compact, anchor, panel, setOpen, 340);
  useEffect(() => {
    if (!anchor.current) return;
    const observer = new ResizeObserver(([entry]) => {
      const next = (entry.borderBoxSize?.[0]?.inlineSize ?? anchor.current!.getBoundingClientRect().width) < 150;
      if (next && document.activeElement === input.current) setOpen(true);
      setCompact(next);
    });
    observer.observe(anchor.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const shortcut = (event:KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "k" || !anchor.current?.getClientRects().length) return;
      event.preventDefault();
      if (compact) setOpen(true); else input.current?.focus();
    };
    window.addEventListener("keydown", shortcut);
    return () => window.removeEventListener("keydown", shortcut);
  }, [compact]);
  useEffect(() => {if (open) input.current?.focus({preventScroll:true});}, [open, compact]);
  useEffect(() => {if (!compact) setOpen(false);}, [compact]);
  const field = <>
    <input ref={input} value={value} onChange={event => onChange(event.target.value)} placeholder={t("modern", "searchLibrary")} aria-label={t("gameGrid", "search")}
      onKeyDown={event => {if (event.key === "Escape") {event.stopPropagation(); onChange(""); if (compact) {setOpen(false); trigger.current?.focus();}}}}/>
    {value ? <button type="button" aria-label={t("gameGrid", "clearSearch")} onClick={() => {onChange(""); input.current?.focus();}}>×</button> : <kbd>Ctrl K</kbd>}
  </>;
  return <div ref={anchor} className="modern-lib-search" data-compact={compact || undefined}>
    <button ref={trigger} type="button" data-dropdown-trigger className="modern-library-search-trigger" title={t("modern", "searchLibrary")} aria-label={t("modern", "searchLibrary")} aria-haspopup={compact ? "dialog" : undefined} aria-expanded={compact ? open : undefined} aria-controls={compact && open ? id : undefined} onClick={() => {if (compact) setOpen(value => !value); else input.current?.focus();}}><MenuIcon name="search"/>{compact && value && <i aria-hidden="true"/>}</button>
    {!compact && field}
    {compact && open && createPortal(<div ref={panel} id={id} role="dialog" aria-label={t("modern", "searchLibrary")} className="modern-library-search-popup" style={position}
      onBlur={event => {if (event.relatedTarget instanceof Node && !panel.current?.contains(event.relatedTarget) && !anchor.current?.contains(event.relatedTarget)) setOpen(false);}}>{field}</div>, document.body)}
  </div>;
}

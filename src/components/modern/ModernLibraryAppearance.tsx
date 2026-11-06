import {useEffect, useId, useRef, useState} from "react";
import {createPortal} from "react-dom";
import {useBoundedDropdown} from "../../hooks/useBoundedDropdown";
import {useI18n} from "../../i18n/I18nContext";
import {defaultLibraryAppearance, libraryCoverSize, updateLibraryAppearance, useLibraryAppearance} from "../../services/libraryAppearance";
import {MenuIcon} from "../MenuIcon";
import "./ModernLibraryAppearance.css";

export function ModernLibraryAppearance() {
  const {t} = useI18n();
  const appearance = useLibraryAppearance();
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null), trigger = useRef<HTMLButtonElement>(null), panel = useRef<HTMLDivElement>(null);
  const id = useId();
  const position = useBoundedDropdown(open, anchor, panel, setOpen, 320, "right");
  useEffect(() => {if (open) panel.current?.querySelector<HTMLInputElement>("input")?.focus({preventScroll:true});}, [open]);
  const close = () => {setOpen(false); trigger.current?.focus({preventScroll:true});};
  return <div ref={anchor} className="modern-library-appearance">
    <button ref={trigger} type="button" data-dropdown-trigger className="modern-lib-icon" title={t("modern", "customizeLibrary")} aria-label={t("modern", "customizeLibrary")} aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => setOpen(value => !value)}><MenuIcon name="sliders"/></button>
    {open && createPortal(<div ref={panel} id={id} className="modern-ui modern-library-appearance-popover" role="dialog" aria-label={t("modern", "customizeLibrary")} style={position}
      onKeyDown={event => {if (event.key === "Escape") {event.preventDefault(); event.stopPropagation(); close();}}}
      onBlur={event => {if (event.relatedTarget instanceof Node && !panel.current?.contains(event.relatedTarget) && !anchor.current?.contains(event.relatedTarget)) setOpen(false);}}>
      <h3>{t("modern", "customizeLibrary")}</h3>
      <section className="library-cover-size"><div><label htmlFor={`${id}-size`}>{t("modern", "coverSize")}</label><output htmlFor={`${id}-size`}>{appearance.coverSize}%</output></div>
        <p id={`${id}-hint`}>{t("modern", "coverSizeHint")}</p>
        <input id={`${id}-size`} type="range" min={libraryCoverSize.min} max={libraryCoverSize.max} step={libraryCoverSize.step} value={appearance.coverSize} aria-describedby={`${id}-hint`} onChange={event => updateLibraryAppearance({coverSize:Number(event.target.value)})}/>
      </section>
      <div className="library-recent-choice"><span>{t("modern", "showRecent")}</span><button type="button" role="switch" aria-label={t("modern", "showRecent")} aria-checked={appearance.showRecent} className={`advanced-toggle${appearance.showRecent ? " enabled" : ""}`} onClick={() => updateLibraryAppearance({showRecent:!appearance.showRecent})}><span/></button></div>
      <footer><button type="button" className="modern-mini-btn" disabled={appearance.coverSize === defaultLibraryAppearance.coverSize && appearance.showRecent === defaultLibraryAppearance.showRecent} onClick={() => updateLibraryAppearance(defaultLibraryAppearance)}>{t("modern", "resetLibraryAppearance")}</button></footer>
    </div>, document.body)}
  </div>;
}

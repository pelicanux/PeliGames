import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "../../components/ModalSurface";
import { useNexusText } from "./text";
import { MenuIcon } from "../../components/MenuIcon";
let nextLayer = 0;
const layers = new Set<number>();
export function NexusPopup({ title, children, onClose, footer, headerActions, onBack, embedded = false, viewKey, className = "" }: { title: string; children: ReactNode; onClose: () => void; footer?: ReactNode; headerActions?: ReactNode; onBack?: () => void; embedded?: boolean; viewKey?: string; className?: string }) {
  const text = useNexusText();
  const titleId = useId();
  const frame = useRef<HTMLDivElement>(null);
  const close = useRef(onClose); close.current = onClose;
  const [layer] = useState(() => ++nextLayer);
  useEffect(() => {
    if (embedded) return;
    layers.add(layer);
    const previous = document.activeElement;
    frame.current?.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
    const top = () => layer === Math.max(...layers);
    const keyboard = (event: KeyboardEvent) => {
      if (!top()) return;
      if (event.target instanceof Element && event.target.closest('.custom-select-menu[role="listbox"]')) return;
      if (event.key === "Escape" && frame.current?.querySelector('[aria-haspopup="listbox"][aria-expanded="true"]')) return;
      if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); close.current(); }
      if (event.key === "Tab") {
        const controls = Array.from(frame.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]') || []).filter(el => el.getClientRects().length);
        const first = controls[0], last = controls[controls.length - 1];
        if (!first) { event.preventDefault(); frame.current?.focus(); }
        else if (event.shiftKey && (document.activeElement === first || !frame.current?.contains(document.activeElement))) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && (document.activeElement === last || !frame.current?.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
      }
    };
    window.addEventListener("keydown", keyboard, true);
    return () => { layers.delete(layer); window.removeEventListener("keydown", keyboard, true); if (previous instanceof HTMLElement && previous.isConnected) previous.focus(); };
  }, [layer, embedded]);
  useEffect(() => { if (viewKey !== undefined) { frame.current?.querySelector(".modal-scroll-body")?.scrollTo({top:0}); frame.current?.querySelector<HTMLElement>("button:not(:disabled)")?.focus(); } }, [viewKey]);
  if (embedded) return <div className={`nexus-inline-view ${className}`}>{children}{footer && <div className="nexus-popup-footer">{footer}</div>}</div>;
  return createPortal(<div className="modal-overlay nexus-popup-overlay" style={{ zIndex: 1200 + layer }} onMouseDown={event => { if (event.target === event.currentTarget && layer === Math.max(...layers)) close.current(); }}>
    <ModalSurface ref={frame} tabIndex={-1} className={`modal-content dialog-glass nexus-popup ${className}`} role="dialog" aria-modal="true" aria-labelledby={titleId}
      headerActions={headerActions} header={<>{onBack && <button type="button" className="nexus-titlebar-back" aria-label={text.back} onClick={onBack}><MenuIcon name="back" /></button>}<h2 id={titleId}><MenuIcon name="puzzle" />{title}</h2></>} onDismiss={() => close.current()} footer={footer ? <div className="nexus-popup-footer">{footer}</div> : undefined}>
      {children}
    </ModalSurface>
  </div>, document.body);
}

import { useEffect, useLayoutEffect, useId, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { ModalSurface } from "../../components/ModalSurface";
import { useNexusText } from "./text";
import { MenuIcon } from "../../components/MenuIcon";
export interface NexusPanelPreview { element: HTMLElement; bounds: DOMRect; scroll: [number, number][] }
let nextLayer = 0;
const layers = new Set<number>();
export function NexusPopup({ title, titleIcon, children, onClose, footer, headerActions, onBack, embedded = false, viewKey, className = "", zoomFrom, zoomPreview, reducedMotion = false }: { title: string; titleIcon?: ReactNode; children: ReactNode; onClose: () => void; footer?: ReactNode; headerActions?: ReactNode; onBack?: () => void; embedded?: boolean; viewKey?: string; className?: string; zoomFrom?: DOMRect; zoomPreview?: () => NexusPanelPreview | null; reducedMotion?: boolean }) {
  const text = useNexusText();
  const titleId = useId();
  const frame = useRef<HTMLDivElement>(null);
  const backdrop = useRef<HTMLDivElement>(null);
  const closing = useRef(false);
  const zoomTransform = () => {
    const bounds = frame.current?.getBoundingClientRect();
    return zoomFrom && bounds ? `translate(${zoomFrom.left - bounds.left}px, ${zoomFrom.top - bounds.top}px) scale(${zoomFrom.width / bounds.width}, ${zoomFrom.height / bounds.height})` : "none";
  };
  useLayoutEffect(() => {
    if (!zoomFrom || reducedMotion || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const animation = frame.current?.animate(
      [{ transform: zoomTransform(), opacity: 1 }, { transform: "none", opacity: 1 }],
      { duration: 420, easing: "cubic-bezier(.4,0,.2,1)", fill: "backwards" },
    );
    const shade = backdrop.current?.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 220, easing: "ease-out", fill: "backwards" });
    // StrictMode replays mount effects. Cancel the first animation before the
    // next measurement, otherwise its scaled bounds become the new origin.
    return () => { animation?.cancel(); shade?.cancel(); };
  }, []);
  const dismiss = () => {
    if (closing.current) return;
    if (!zoomFrom || reducedMotion || window.matchMedia("(prefers-reduced-motion: reduce)").matches || !frame.current) { onClose(); return; }
    closing.current = true;
    const source = zoomPreview?.();
    let preview: HTMLDivElement | undefined;
    let animation: Animation;
    if (source && backdrop.current?.parentElement) {
      const from = frame.current.getBoundingClientRect();
      const to = source.bounds;
      // A DOM snapshot keeps the small panel's exact layout and scroll position,
      // instead of shrinking the expanded window's different arrangement.
      const copy = source.element.cloneNode(true) as HTMLElement;
      copy.removeAttribute("id");
      copy.querySelectorAll("[id]").forEach(node => node.removeAttribute("id"));
      const copies = [copy, ...copy.querySelectorAll<HTMLElement>("*")];
      Object.assign(copy.style, { width: `${to.width}px`, height: `${to.height}px`, minHeight: "0", maxHeight: "none", margin: "0", transform: "none" });
      preview = document.createElement("div");
      preview.className = "nexus-panel nexus-carousel-panels nexus-return-preview";
      preview.setAttribute("aria-hidden", "true");
      preview.inert = true;
      Object.assign(preview.style, { left: `${from.left}px`, top: `${from.top}px`, width: `${to.width}px`, height: `${to.height}px` });
      preview.append(copy);
      backdrop.current.parentElement.append(preview);
      copies.forEach((node, index) => { node.scrollTop = source.scroll[index]?.[0] ?? 0; node.scrollLeft = source.scroll[index]?.[1] ?? 0; });
      // Remove the live expanded surface from layout and painting immediately.
      // Visibility alone can leave composited text/backdrop layers visible in WebKit.
      frame.current.style.display = "none";
      animation = preview.animate([
        { transform: `scale(${from.width / to.width}, ${from.height / to.height})`, opacity: 1 },
        { transform: `translate(${to.left - from.left}px, ${to.top - from.top}px) scale(1)`, opacity: 1 },
      ], { duration: 280, easing: "cubic-bezier(.4,0,.2,1)", fill: "forwards" });
    } else {
      animation = frame.current.animate([{ transform: "none", opacity: 1 }, { transform: zoomTransform(), opacity: .3 }], { duration: 280, easing: "cubic-bezier(.4,0,.2,1)", fill: "forwards" });
    }
    const shade = backdrop.current?.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 280, easing: "ease-out", fill: "forwards" });
    void Promise.all([animation.finished, shade?.finished]).then(() => { preview?.remove(); onClose(); }).catch(() => { preview?.remove(); onClose(); });
  };
  const close = useRef(dismiss); close.current = dismiss;
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
  return createPortal(<div className={`modal-overlay nexus-popup-overlay${zoomFrom ? " nexus-zoom-overlay" : ""}`} style={{ zIndex: 1200 + layer }} onMouseDown={event => { if (event.target === event.currentTarget && layer === Math.max(...layers)) close.current(); }}>
    {zoomFrom && <div ref={backdrop} className="nexus-zoom-backdrop" aria-hidden="true" />}
    <ModalSurface ref={frame} tabIndex={-1} className={`modal-content dialog-glass nexus-popup ${className}`} role="dialog" aria-modal="true" aria-labelledby={titleId}
      headerActions={headerActions} header={<>{onBack && <button type="button" className="nexus-titlebar-back" aria-label={text.back} onClick={zoomFrom ? () => close.current() : onBack}><MenuIcon name="back" /></button>}<h2 id={titleId}>{titleIcon ?? <MenuIcon name="puzzle" />}{title}</h2></>} onDismiss={() => close.current()} footer={footer ? <div className="nexus-popup-footer">{footer}</div> : undefined}>
      {children}
    </ModalSurface>
  </div>, document.body);
}

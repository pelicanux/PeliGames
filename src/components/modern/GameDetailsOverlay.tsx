import { useLayoutEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { motion, useReducedMotion } from "framer-motion";

/** The library stays mounted and at its scroll position behind the game. */
export function GameDetailsOverlay({ children, label, closeLabel, onClose, reducedMotion, catalog, flying }: {
  children: ReactNode; label: string; closeLabel: string; onClose: () => void; reducedMotion: boolean; catalog: boolean; flying: boolean;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const prefersReducedMotion = useReducedMotion();
  useLayoutEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const library = document.querySelector<HTMLElement>(".modern-shell");
    const wasInert = library?.inert ?? false;
    if (library) library.inert = true;
    frame.current?.focus({ preventScroll: true });
    return () => {
      if (library) library.inert = wasInert;
      if (previous?.isConnected) previous.focus({ preventScroll: true });
    };
  }, []);
  return createPortal(<motion.div className={`modern-ui game-details-overlay${flying ? " cover-flight-active" : ""}`}
    onClick={event => { if (event.target === event.currentTarget) onClose(); }}
    initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
    transition={{ duration: reducedMotion || prefersReducedMotion ? 0 : .18 }}>
    <div ref={frame} className={`game-details-dialog${catalog ? " game-details-dialog-catalog" : ""}`} role="dialog" aria-modal="true" aria-label={label} tabIndex={-1}
      onKeyDown={event => {
        // Child dialogs and dropdowns use portals and manage their own focus.
        if (!frame.current?.contains(event.target as Node) || event.defaultPrevented) return;
        if (event.key === "Escape") { event.stopPropagation(); onClose(); }
        if (event.key === "Tab") {
          const controls = Array.from(frame.current.querySelectorAll<HTMLElement>('button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]')).filter(el => el.getClientRects().length > 0);
          const first = controls[0], last = controls[controls.length - 1];
          if (event.shiftKey && (document.activeElement === first || document.activeElement === frame.current)) { event.preventDefault(); last?.focus({ preventScroll: true }); }
          else if (!event.shiftKey && (document.activeElement === last || document.activeElement === frame.current)) { event.preventDefault(); first?.focus({ preventScroll: true }); }
        }
      }}>
      <button type="button" className="game-details-dismiss" aria-label={closeLabel} title={closeLabel} onClick={onClose}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18"/></svg>
      </button>
      <div className="game-details-scroll">{children}</div>
    </div>
  </motion.div>, document.body);
}

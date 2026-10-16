import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { useI18n } from "../../i18n/I18nContext";

/** Keep every library source reachable when the toolbar has limited room. */
export function ModernLibraryFilters({ children }: { children: ReactNode }) {
  const { language, t } = useI18n();
  const id = useId();
  const viewport = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const [edges, setEdges] = useState({ left: false, right: false });
  const update = () => {
    const element = viewport.current;
    if (!element) return;
    const left = element.scrollLeft > 1;
    const right = element.scrollWidth - element.clientWidth - element.scrollLeft > 1;
    setEdges(current => current.left === left && current.right === right ? current : { left, right });
  };
  useEffect(() => {
    const observer = new ResizeObserver(update);
    if (viewport.current) observer.observe(viewport.current);
    if (content.current) observer.observe(content.current);
    update();
    return () => observer.disconnect();
  }, [children]);
  const move = (direction: number) => {
    const element = viewport.current;
    if (!element) return;
    element.scrollBy({ left: direction * Math.max(120, element.clientWidth * .8),
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth" });
  };
  const overflow = edges.left || edges.right;
  return <div className="modern-lib-filter-strip">
    {overflow && <button type="button" className="modern-lib-filter-arrow" aria-label={language === "pt" ? "Categorias anteriores" : "Previous categories"} aria-controls={id} disabled={!edges.left} onClick={() => move(-1)}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m14 6-6 6 6 6" /></svg>
    </button>}
    <div ref={viewport} id={id} className="modern-lib-chips" role="group" aria-label={t("gameGrid", "viewMode")} onScroll={update}>
      <div ref={content} className="modern-lib-chip-content">{children}</div>
    </div>
    {overflow && <button type="button" className="modern-lib-filter-arrow" aria-label={language === "pt" ? "Próximas categorias" : "Next categories"} aria-controls={id} disabled={!edges.right} onClick={() => move(1)}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m10 6 6 6-6 6" /></svg>
    </button>}
  </div>;
}

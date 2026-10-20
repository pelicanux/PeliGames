import {GameSaveStatusProvider} from "../GameSaveStatus";
import { CatalogNavigationContext } from "./CatalogNavigationContext";
import { HoverTooltip } from "../HoverTooltip";
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";

export interface ModernTab { id: string; label: string; content: ReactNode; count?: number; disabled?: boolean; disabledHint?: string }

interface Props {
  /** Game artwork behind the spotlight (shaded where the text sits); later
   *  images show through when an earlier one fails to load. */
  artwork?: string[];
  /** Wide store artwork shows sharp; a portrait cover is blurred into colour. */
  artSharp?: boolean;
  cover?: ReactNode;
  eyebrow?: ReactNode;
  title?: ReactNode;
  meta?: ReactNode;
  actions?: ReactNode;
  /** Replaces cover, title and actions: installation and empty states. */
  summary?: ReactNode;
  tabs: ModernTab[];
  catalog?: ReactNode;
  contentView?: ReactNode;
  /** Changing it (another game or mode) returns to the first tab. */
  resetKey: string;
  activeTab?: string;
  onTabChange?: (tab: string) => void;
  requestedTab?: string;
  tabRequest?: {tab: string; revision: number};
  onClose: () => void;
  closeLabel: string;
}

/** The selected game in the new interface: a spotlight with the game's
 *  artwork and main actions, then its panels as tabs. */
export function ModernGameDetails({ artwork, artSharp = false, cover, eyebrow, title, meta, actions, summary, tabs, catalog, contentView, resetKey, activeTab, onTabChange, requestedTab, tabRequest, onClose, closeLabel }: Props) {
  const frame = useRef<HTMLElement>(null);
  const [navigationHost, setNavigationHost] = useState<HTMLDivElement | null>(null);
  const [artHeight, setArtHeight] = useState<number>();
  useLayoutEffect(() => {
    const element = frame.current;
    if (!element) return;
    // The artwork is absolute, so measuring the panel does not feed back into
    // its layout. A numeric height lets CSS interpolate background-cover resize.
    const measure = () => setArtHeight(element.getBoundingClientRect().height);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const browsing = Boolean(catalog);
  useLayoutEffect(() => {
    if (browsing) frame.current?.closest(".game-details-scroll, .container.modern-scroll")?.scrollTo({top: 0});
  }, [browsing]);
  const [tab, setTab] = useState(tabs[0]?.id);
  useEffect(() => { setTab(tabs[0]?.id); }, [resetKey]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (requestedTab && tabs.some(item => item.id === requestedTab)) { setTab(requestedTab); onTabChange?.(requestedTab); } }, [requestedTab, resetKey]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { if (tabRequest?.revision && tabs.some(item => item.id === tabRequest.tab)) { setTab(tabRequest.tab); onTabChange?.(tabRequest.tab); } }, [tabRequest]); // eslint-disable-line react-hooks/exhaustive-deps
  const current = tabs.find(item => !item.disabled && item.id === (activeTab ?? tab)) ?? tabs.find(item => !item.disabled);
  return <GameSaveStatusProvider key={resetKey}><section ref={frame} className={`modern-details${contentView ? " modern-details-secondary" : ""}${catalog ? " modern-details-browsing" : ""}${tabs[0]?.id === "installation" ? " modern-details-installation" : ""}`} aria-label={typeof title === "string" ? title : undefined}>
    <div className={`modern-details-art${artSharp ? " sharp" : ""}`} style={{
      ...(artwork?.length ? { backgroundImage: artwork.map(url => `url("${url.replace(/"/g, "%22")}")`).join(", ") } : {}),
      ...(artHeight !== undefined ? { "--modern-art-height": `${artHeight}px` } : {}),
    } as CSSProperties} aria-hidden="true" />
    <button type="button" className="modern-details-close" aria-label={closeLabel} title={closeLabel} onClick={onClose}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18"/></svg>
    </button>
    {contentView ?? <>{catalog && <div ref={setNavigationHost} className="modern-catalog-navigation" />}
    {summary ? <div className="modern-details-summary">{summary}</div> : <div className="modern-details-top">
      <div className="modern-details-cover">{cover}</div>
      <div className="modern-details-info">
        {eyebrow && <div className="modern-details-eyebrow">{eyebrow}</div>}
        {title && <h1>{title}</h1>}
        {meta && <div className="modern-details-meta">{meta}</div>}
      </div>
      {actions && <div className="modern-details-actions" hidden={Boolean(catalog)}>{actions}</div>}
    </div>}
    {!catalog && tabs.length > 1 && <div className="modern-tabs" role="tablist">
      {tabs.map(item => item.disabled ? <HoverTooltip key={item.id} text={item.disabledHint || "Em desenvolvimento"}><button type="button" role="tab" disabled aria-selected={false} style={{opacity:0.45,cursor:"not-allowed"}}>{item.label}</button></HoverTooltip> : <button type="button" key={item.id} role="tab" aria-selected={current?.id === item.id} onClick={() => { setTab(item.id); onTabChange?.(item.id); }}>
        {item.label}{item.count ? <span className="modern-tab-count">{item.count}</span> : null}
      </button>)}
    </div>}
    {catalog ? <CatalogNavigationContext.Provider value={navigationHost}><div className="modern-catalog-scroll modern-details-catalog" data-scrollbar-persistent="true">{catalog}</div></CatalogNavigationContext.Provider> : <div className="modern-tab-body" role={tabs.length > 1 ? "tabpanel" : undefined}>{current?.content}</div>}
    </>}
  </section></GameSaveStatusProvider>;
}

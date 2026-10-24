import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useI18n } from "../../i18n/I18nContext";
import { ModernNavIcon, type ModernIconName } from "./ModernNavIcon";

export type ModernSection = "library" | "install" | "dlss" | "nexus" | "downloads" | "community" | "discord" | "donate" | "youtube" | "settings";
/** Settings pages listed in the side menu while Settings is open. */
export type ModernSettingsPage = "interface" | "mods" | "protons" | "covers" | "connections" | "monitoring" | "diskAutomount" | "tools";

interface Props {
  active: ModernSection;
  disabled?: boolean;
  onNavigate: (section: ModernSection) => void;
  /** Number of active Nexus downloads, shown on the Downloads item. */
  activeDownloads?: number;
  /** Number of games in the library, shown on the Library item. */
  libraryCount?: number;
  /** While Settings is open the menu lists its pages instead. */
  settingsPage?: ModernSettingsPage | null;
  onSettingsNavigate?: (page: ModernSettingsPage) => void;
  children: ReactNode;
}

const COLLAPSED_KEY = "peligames.modernMenuCollapsed";
type MenuMode = "full" | "rail";

/** Side navigation switches between the full menu and an icon rail. */
export function ModernShell({ active, disabled = false, onNavigate, activeDownloads = 0, libraryCount, settingsPage = null, onSettingsNavigate, children }: Props) {
  const { t } = useI18n();
  const frame = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(() => window.innerWidth);
  const [collapsed, setCollapsed] = useState<boolean | null>(() => { try { const value = localStorage.getItem(COLLAPSED_KEY); return value === null ? null : value === "1"; } catch { return null; } });
  const [animate, setAnimate] = useState(false);
  const [pointerInside, setPointerInside] = useState(false);
  const [keyboardInside, setKeyboardInside] = useState(false);
  useEffect(() => {
    const close = () => { setPointerInside(false); setKeyboardInside(false); };
    const leaveWindow = (event: MouseEvent) => { if (!event.relatedTarget) close(); };
    const visibility = () => { if (document.hidden) close(); };
    // WebKit can retain CSS :hover when the pointer leaves the native window.
    window.addEventListener("blur", close);
    window.addEventListener("mouseout", leaveWindow);
    document.documentElement.addEventListener("pointerleave", close);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      window.removeEventListener("blur", close);
      window.removeEventListener("mouseout", leaveWindow);
      document.documentElement.removeEventListener("pointerleave", close);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, []);
  useLayoutEffect(() => {
    const element = frame.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(element);
    // The first layout snaps into place; later width changes animate.
    const raf = requestAnimationFrame(() => setAnimate(true));
    return () => { observer.disconnect(); cancelAnimationFrame(raf); };
  }, []);
  // Keep navigation available as an icon rail even in narrow windows.
  const mode: MenuMode = width < 760 || (collapsed ?? width < 1100) ? "rail" : "full";
  const toggleCollapsed = () => {
    const next = mode === "full";
    setCollapsed(next);
    try { localStorage.setItem(COLLAPSED_KEY, next ? "1" : "0"); } catch { /* Session only. */ }
  };
  const button = (key: string, current: boolean, icon: ModernIconName, label: string, hint: string, onClick: () => void, badge?: number) =>
    <button type="button" key={key} className="modern-nav" title={label} aria-current={current ? "page" : undefined} disabled={disabled} onClick={onClick}>
      <ModernNavIcon name={icon} />
      <span className="modern-nav-label">{label}<small>{hint}</small></span>
      {badge ? <span className="modern-nav-badge">{badge}</span> : null}
    </button>;
  const item = (section: ModernSection, icon: ModernIconName, label: string, hint: string, badge?: number) =>
    button(section, active === section, icon, label, hint, () => onNavigate(section), badge);
  const page = (id: ModernSettingsPage, icon: ModernIconName, label: string, hint: string) =>
    button(id, settingsPage === id, icon, label, hint, () => onSettingsNavigate?.(id));

  return <div ref={frame} className={`modern-shell modern-menu-${mode}${animate ? " modern-animate" : ""}`}>
    <div className="modern-side-slot">
      <nav className={`modern-sidebar${pointerInside || keyboardInside ? " modern-peek-open" : ""}`} aria-label="PeliGames"
        onPointerEnter={() => setPointerInside(true)} onPointerMove={() => setPointerInside(true)} onPointerLeave={() => setPointerInside(false)}
        onFocusCapture={event => setKeyboardInside(event.target.matches(":focus-visible"))}
        onBlurCapture={event => { if (!event.currentTarget.contains(event.relatedTarget)) setKeyboardInside(false); }}
        onKeyDownCapture={event => { if (event.key === "Tab") setKeyboardInside(true); }}>
        {settingsPage ? <>
          {button("back", false, "library", t("modern", "library"), t("modern", "backToGames"), () => onNavigate("library"))}
          <h6><span>{t("modern", "settings")}</span></h6>
          {page("interface", "interface", t("modern", "interfacePage"), t("modern", "interfaceHint"))}
          {page("mods", "mods", t("modern", "modsPage"), t("modern", "modsPageHint"))}
          {page("protons", "proton", "Proton", t("modern", "protonHint"))}
          {page("covers", "covers", t("modern", "coversPage"), "SteamGridDB")}
          {page("connections", "connections", t("modern", "connectionsPage"), "Nexus, Discord, Matrix")}
          <h6><span>{t("modern", "toolsGroup")}</span></h6>
          {page("monitoring", "monitoring", t("modern", "monitoringPage"), "MangoHUD")}
          {page("diskAutomount", "disks", t("modern", "disksPage"), t("modern", "disksHint"))}
          {page("tools", "logs", t("modern", "logsPage"), t("modern", "logsHint"))}
        </> : <>
          <h6><span>{t("modern", "play")}</span></h6>
          {item("library", "library", t("modern", "library"), t("modern", "libraryHint"), libraryCount)}
          {item("install", "install", t("modern", "install"), t("modern", "installHint"))}
          {item("downloads", "downloads", t("modern", "downloads"), t("modern", "downloadsHint"), activeDownloads)}
        </>}
        <h6><span>{t("modern", "community")}</span></h6>
        {item("discord", "discord", "Discord", t("modern", "discordHint"))}
        {item("community", "matrix", t("modern", "matrixChat"), t("modern", "matrixHint"))}
        {item("donate", "donate", t("credits", "support"), "LivePix")}
        {item("youtube", "youtube", t("modern", "membership"), "YouTube")}
        {!settingsPage && <><h6><span>{t("modern", "peligamesGroup")}</span></h6>{item("settings", "settings", t("modern", "settings"), t("modern", "settingsHint"))}</>}
        <div className="modern-side-foot">
          <button type="button" className="modern-nav modern-collapse" onClick={toggleCollapsed}
            title={t("modern", mode === "rail" ? "expand" : "collapse")}>
            <ModernNavIcon name="collapse" />
            <span className="modern-nav-label">{t("modern", mode === "rail" ? "expand" : "collapse")}</span>
          </button>
        </div>
      </nav>
    </div>
    <div className="modern-main">{children}</div>
  </div>;
}

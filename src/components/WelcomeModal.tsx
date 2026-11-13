import {AddonGuideVisual} from "../modules/addons/AddonGuide";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { BrandName } from "./BrandName";
import { SelectionMenu } from "./SelectionMenu";
import { useI18n } from "../i18n/I18nContext";
import {useLanguageChoices} from "../i18n/useLanguageChoices";
import { notifyError } from "../services/notifications";
import { welcomeSlides, type WelcomeDestination } from "./welcomeTour";
import "./WelcomeModal.css";

interface AppImageStatus { appimage: boolean; installed: boolean; target: string }
export function WelcomeModal({ onStart, onNavigate }: { onStart: () => void; onNavigate: (destination: WelcomeDestination) => void }) {
  const {t: tx} = useI18n();

  const {t, language, preference, options, loading, setLanguage} = useLanguageChoices();
  const [status, setStatus] = useState<AppImageStatus | null>(null);
  const offerInstall = Boolean(status?.appimage && !status.installed);
  const slides = welcomeSlides(language, offerInstall);
  const [step, setStep] = useState(0);
  const slide = slides[step], last = step === slides.length - 1, intro = slide.id === "welcome";
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState("");
  const root = useRef<HTMLDivElement>(null), heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    let active = true;
    void invoke<AppImageStatus>("appimage_install_status").then(value => { if (active) setStatus(value); }).catch(() => { if (active) setStatus({ appimage: false, installed: false, target: "" }); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    root.current?.querySelector(".modal-scroll-body")?.scrollTo({ top: 0 });
    heading.current?.focus({ preventScroll: true });
  }, [step, status !== null]);
  useEffect(() => {
    if (!status || !import.meta.env.DEV || !("__PELI_UI_PREVIEW__" in window)) return;
    const requested = Number(new URLSearchParams(location.search).get("welcomeStep"));
    if (Number.isInteger(requested) && requested >= 1) setStep(Math.min(requested, slides.length) - 1);
  }, [status]);
  const install = async () => {
    if (installing) return;
    setInstalling(true); setError("");
    try { localStorage.setItem("peligames_welcome_completed", "1"); } catch { /* The installed copy shows the welcome again. */ }
    try { await invoke("install_peligames_appimage"); }
    catch (e) { setError(String(e)); notifyError(t("welcome", "installTitle"), e); setInstalling(false); }
  };
  if (!status) return <div className="modal-overlay welcome-overlay"><ModalSurface hideClose className="modal-content dialog-glass" role="status">{tx("ui-welcome-modal", "preparing_your_welcome")}</ModalSurface></div>;
  return <div className="modal-overlay welcome-overlay">
    <ModalSurface ref={root} hideClose className="modal-content dialog-glass welcome-tour welcome-tour-modern" role="dialog" aria-modal="true" aria-labelledby="launcher-welcome-title"
      onKeyDown={event => {
        if (event.key === "Escape" && !installing) { event.stopPropagation(); onStart(); }
        if (event.key !== "Tab") return;
        const items = Array.from(root.current?.querySelectorAll<HTMLElement>('button:not(:disabled), [href], [tabindex="0"]') ?? []).filter(item => item.getClientRects().length);
        const first = items[0], end = items[items.length - 1];
        if (event.shiftKey && (document.activeElement === first || document.activeElement === heading.current)) { event.preventDefault(); end?.focus(); }
        else if (!event.shiftKey && document.activeElement === end) { event.preventDefault(); first?.focus(); }
      }}
      header={<div className="welcome-brand"><img src="/peligames.svg" alt="" /><span className="titlebar-title"><BrandName /></span><span>{tx("ui-welcome-modal", "getting_started")}</span></div>}
      headerActions={<SelectionMenu className="welcome-language" label={t("settings", "language")} value={preference}
        leadingIcon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><circle cx="12" cy="12" r="10" /><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20" /></svg>}
        options={options} disabled={installing || loading}
        onChange={value => void setLanguage(value)} />}
      footer={<footer className="welcome-tour-footer">
        <button type="button" className="welcome-skip" disabled={installing} onClick={onStart}>{tx("ui-welcome-modal", "explore_on_my_own")}</button>
        <nav className="welcome-pagination" aria-label={tx("ui-welcome-modal", "tour_steps")}>{slides.map((item, index) => <button key={item.id} type="button" disabled={installing} aria-label={`${index + 1}. ${item.title}`} aria-current={step === index ? "step" : undefined} onClick={() => setStep(index)} />)}</nav>
        <div className="welcome-navigation"><span className="welcome-step-count">{step + 1} / {slides.length}</span>
          <button type="button" className="btn btn-secondary" disabled={!step || installing} onClick={() => setStep(value => value - 1)}>{tx("ui-welcome-modal", "back")}</button>
          <button type="button" className="btn btn-primary" disabled={installing} onClick={() => last ? onStart() : setStep(value => value + 1)}>{last ? tx("ui-welcome-modal", "get_started") : tx("ui-welcome-modal", "next")}</button>
        </div>
      </footer>}>
      <section key={slide.id} className={`welcome-slide ${intro ? "welcome-slide-intro" : ""} ${slide.id === "appimage" ? "welcome-slide-appimage" : ""}`}>
        <div className="welcome-copy">
          <span className="welcome-eyebrow">{intro ? "PELICANUX / PELIGAMES" : slide.id === "appimage" ? "APPIMAGE / PELIGAMES" : tx("ui-welcome-modal", "discover_peligames")}</span>
          <h2 ref={heading} tabIndex={-1} id="launcher-welcome-title">{slide.title}</h2>
          <p className="welcome-description">{slide.description}</p>
          <ul className="welcome-points">{slide.points.map(point => <li key={point}><span aria-hidden="true" />{point}</li>)}</ul>
          {slide.id === "appimage" && <div className="welcome-install"><strong>{tx("ui-welcome-modal", "installation_folder")}</strong><code>{status.target}</code><button type="button" className="btn btn-primary" disabled={installing} onClick={() => void install()}><MenuIcon name="download" />{installing ? t("welcome", "installing") : t("welcome", "installButton")}</button></div>}
          {slide.destination && <button type="button" className="btn btn-secondary welcome-shortcut" disabled={installing} onClick={() => onNavigate(slide.destination!)}>{slide.action}<MenuIcon name="folderSearch" /></button>}
          {last && <button type="button" className="welcome-community-link" disabled={installing} onClick={() => onNavigate("matrix")}>{tx("ui-welcome-modal", "explore_matrix_chat")} <span aria-hidden="true">↗</span></button>}
          {error && <p role="alert" className="preferences-error">{error}</p>}
        </div>
        <figure className={`welcome-art ${intro ? "welcome-art-stack" : ""}`}>
          {slide.id==="addons"?<AddonGuideVisual/>:<div className="welcome-screens">
            {intro && <><img className="welcome-screen welcome-screen-back" src="/welcome/monitoring.jpg" alt="" /><img className="welcome-screen welcome-screen-middle" src="/welcome/nexus.jpg" alt="" /></>}
            <img className="welcome-screen welcome-screen-front" src={`/welcome/${slide.image}.jpg`} alt={slide.alt} />
          </div>}
          <figcaption>{tx("ui-welcome-modal", "interface_screenshots_demonstration_data")}</figcaption>
        </figure>
      </section>
    </ModalSurface>
  </div>;
}

import {useEffect, useId, useLayoutEffect, useRef, useState} from "react";
import {createPortal} from "react-dom";
import {MenuIcon} from "../../components/MenuIcon";
import {ModalSurface} from "../../components/ModalSurface";
import {ModDetailHeader} from "../../components/ModDetailHeader";
import {useI18n} from "../../i18n/I18nContext";
import {AddonStore} from "./AddonStore";
import {AddonGuide} from "./AddonGuide";
import {OPEN_MATRIX_CHAT_EVENT} from "../../services/matrixChat";
import {AddonView} from "./AddonView";
import {addonStore, useAddonStore} from "./store";
import type {AddonRoute} from "./contracts";
import "./storeWindow.css";

/** The catalog and configuration share one dialog and one package registry. */
export function AddonStoreWindow({onClose,initialGuide=false}: {onClose: () => void;initialGuide?:boolean}) {
  const {t} = useI18n();
  const title = useId();
  const frame = useRef<HTMLDivElement>(null);
  const [showGuide,setShowGuide]=useState(initialGuide);
  const [route, setRoute] = useState<AddonRoute | null>(null);
  const [viewBusy, setViewBusy] = useState(false);
  const snapshot = useAddonStore();
  const busy = Boolean(snapshot.busy && snapshot.busy !== "catalog") || viewBusy;
  const latest = useRef({busy, onClose});
  latest.current = {busy, onClose};
  const dismiss = () => {
    const operation = addonStore.getSnapshot().busy;
    if (!latest.current.busy && (!operation || operation === "catalog")) latest.current.onClose();
  };

  useEffect(() => {
    const openChat = () => dismiss();
    window.addEventListener(OPEN_MATRIX_CHAT_EVENT, openChat);
    return () => window.removeEventListener(OPEN_MATRIX_CHAT_EVENT, openChat);
  }, []);

  useLayoutEffect(() => {
    const previous = document.activeElement;
    const launcher = document.querySelector<HTMLElement>(".app-wrapper");
    const wasInert = launcher?.inert ?? false;
    if (launcher) launcher.inert = true;
    frame.current?.focus({preventScroll: true});
    return () => {
      if (launcher) launcher.inert = wasInert;
      requestAnimationFrame(() => {if (previous instanceof HTMLElement && previous.isConnected) previous.focus({preventScroll: true});});
    };
  }, []);
  useEffect(() => {
    frame.current?.querySelector(".modal-scroll-body")?.scrollTo({top: 0});
    frame.current?.focus({preventScroll: true});
  }, [route?.id,showGuide]);

  return createPortal(<div className="modal-overlay modern-ui addon-store-overlay" onClick={event => {if (event.target === event.currentTarget) dismiss();}}>
    <ModalSurface ref={frame} className="modal-content dialog-glass modern-dialog addon-store-window" role="dialog" aria-modal="true" aria-labelledby={title} tabIndex={-1}
      onDismiss={dismiss} closeDisabled={busy}
      header={<h2 id={title}><MenuIcon name="addonStore"/>{t("addons", "store")}</h2>}
      headerActions={!showGuide&&!route?<button type="button" className="addon-guide-help" disabled={busy} onClick={()=>setShowGuide(true)}><MenuIcon name="info"/>{t("addons","guide_help")}</button>:undefined}
      onKeyDown={event => {
        // Portaled selection menus manage their own focus and Escape handling.
        if (!frame.current?.contains(event.target as Node) || event.defaultPrevented) return;
        if (event.key === "Escape") {event.stopPropagation(); dismiss();}
        if (event.key === "Tab") {
          const controls = Array.from(frame.current.querySelectorAll<HTMLElement>('button:not(:disabled), a[href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex="0"]')).filter(item => item.getClientRects().length);
          const first = controls[0], last = controls[controls.length - 1];
          if (!first) {event.preventDefault(); return;}
          if (event.shiftKey && (document.activeElement === first || document.activeElement === frame.current)) {event.preventDefault(); last.focus();}
          else if (!event.shiftKey && (document.activeElement === last || document.activeElement === frame.current)) {event.preventDefault(); first.focus();}
        }
      }}>
      {showGuide&&<AddonGuide onOpenHub={()=>setShowGuide(false)} onSkip={dismiss}/>}
      <div hidden={Boolean(route)||showGuide}><AddonStore onOpen={setRoute}/></div>
      {!showGuide&&route && <>
        <ModDetailHeader title={route.project?.name || route.title || route.id} project={route.project} backLabel={t("addons", "back")} onBack={() => setRoute(null)} disabled={busy}/>
        <AddonView route={route} onBusyChange={setViewBusy}/>
      </>}
    </ModalSurface>
  </div>, document.body);
}

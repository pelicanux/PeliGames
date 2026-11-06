import {openFrameGenerationPreferences} from "../services/frameGenerationNavigation";
import { openMangoHudInstallation } from "../services/mangohud";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";
import type { LauncherUpdateInfo } from "../services/launcherUpdates";
import type { ProtonFamily } from "../services/protonService";
import "./notifications.css";
import { LAUNCHER_ERROR_EVENT, LAUNCHER_NOTICE_EVENT, repeatNotification, type LauncherErrorDetail } from "../services/notifications";
import { MATRIX_MENTION_EVENT, MATRIX_READ_EVENT, OPEN_MATRIX_CHAT_EVENT, type MatrixMentionDetail, type MatrixReadDetail } from "../services/matrixChat";
import { playNotificationSound, unlockNotificationSound } from "../services/notificationSound";
import { useNativeVideoOverlay } from "../services/matrixScreenView";

export type UpdateTarget = "launcher" | "rdna4" | "rdna3" | ProtonFamily;
type Notice = { id: string; target: UpdateTarget | "nexus" | "error" | "info" | "matrix"; version: string; message?: string; source?: string; roomId?: string; eventId?: string; action?: LauncherErrorDetail["action"]; unread: boolean };
type Store = { items: Notice[]; seen: string[] };
type BackendStatus = { latest: string; current: string | null; needs_update: boolean };
const storageKey = "peligames_update_notifications_v1";
function readStore(): Store {
  try {
    const value = JSON.parse(localStorage.getItem(storageKey) || "null");
    if (Array.isArray(value?.items) && Array.isArray(value?.seen)) return {
      items: value.items.filter((n: Notice) => n && ["launcher", "rdna3", "rdna4", "ge-proton", "cachyos-proton", "nexus", "error", "info", "matrix"].includes(n.target) && (!["nexus", "error", "info", "matrix"].includes(n.target) || typeof n.message === "string") && typeof n.id === "string" && typeof n.version === "string").slice(0, 20),
      seen: value.seen.filter((id: unknown) => typeof id === "string").slice(-100),
    };
  } catch { /* Storage may be unavailable; notifications still work in memory. */ }
  return { items: [], seen: [] };
}
const Bell = () => <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4"/></svg>;

export function NotificationCenter({ launcherUpdate, onNavigate, onNexusAccount, nexusError = "", navigationBusy = false }: { nexusError?: string; navigationBusy?: boolean; launcherUpdate: LauncherUpdateInfo | null; onNavigate: (target: UpdateTarget) => void; onNexusAccount: () => void }) {

  const {t: tx,  language } = useI18n();

  const reducedMotion = useReducedMotion();
  const [store, setStore] = useState(readStore);
  const storeRef = useRef(store);
  const protonResponses = useRef(new Set<ProtonFamily>());
  const [open, setOpen] = useState(false);
  const openRef = useRef(false);
  const [toasts, setToasts] = useState<Notice[]>([]);
  const [toastHovered, setToastHovered] = useState(false);
  const [toastFocused, setToastFocused] = useState(false);
  const toastTime = useRef({ id: "", remaining: 3000 });
  const panelRef = useRef<HTMLElement>(null);
  const toastRef = useRef<HTMLButtonElement>(null);
  useNativeVideoOverlay(panelRef);
  useNativeVideoOverlay(toastRef);
  const contentRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [listHeight, setListHeight] = useState<number | undefined>();
  const [panelHeight, setPanelHeight] = useState<number | null>(null);
  const bellRef = useRef<HTMLButtonElement>(null);
  const commit = (next: Store) => {
    storeRef.current = next;
    setStore(next);
    try { localStorage.setItem(storageKey, JSON.stringify(next)); } catch { /* In-memory fallback. */ }
  };
  const reconcile = (target: UpdateTarget, version: string | null) => {
    const previous = storeRef.current;
    const other = previous.items.filter(n => n.target !== target);
    if (!version) {
      commit({ ...previous, items: other });
      setToasts(queue => queue.filter(n => n.target !== target));
      return;
    }
    const id = `${target}:${version}`;
    const existing = previous.items.find(n => n.id === id);
    const seen = previous.seen.includes(id);
    if (!existing && seen) { commit({ ...previous, items: other }); return; }
    const notice: Notice = existing || { id, target, version, unread: !openRef.current };
    commit({ items: existing ? previous.items.map(n => n.id === id ? notice : n).filter(n => n.target !== target || n.id === id) : [notice, ...other], seen: seen ? previous.seen : [...previous.seen, id].slice(-100) });
    if (!seen && !openRef.current) setToasts(queue => [...queue.filter(n => n.target !== target), notice]);
  };
  const showFeedback = (notice: Notice) => {
    commit({ ...storeRef.current, items: repeatNotification(storeRef.current.items, notice).slice(0, 20) });
    // A fresh action must show its result even after the bell was opened.
    openRef.current = false;
    setOpen(false);
    setToasts(queue => repeatNotification(queue, notice));
  };
  const notifyNexusError = (message: string) => {
    if (!message.trim()) return;
    showFeedback({ id: `nexus:${crypto.randomUUID()}`, target: "nexus", version: "", message, unread: true });
  };
  useEffect(() => { notifyNexusError(nexusError); }, [nexusError]);
  // Any launcher error (notifyError): kept like the Nexus ones, one entry per message.
  useEffect(() => {
    const received = (event: Event) => {
      const { source, message, action } = (event as CustomEvent<LauncherErrorDetail>).detail;
      if (!message?.trim()) return;
      const target = event.type === LAUNCHER_NOTICE_EVENT ? "info" : "error";
      showFeedback({ id: `${target}:${crypto.randomUUID()}`, target, version: "", message, source, action, unread: true });
    };
    window.addEventListener(LAUNCHER_ERROR_EVENT, received);
    window.addEventListener(LAUNCHER_NOTICE_EVENT, received);
    return () => { window.removeEventListener(LAUNCHER_ERROR_EVENT, received); window.removeEventListener(LAUNCHER_NOTICE_EVENT, received); };
  }, []);
  // Someone mentioned the user in the Matrix chat: one entry per message, opens the chat.
  useEffect(() => {
    document.addEventListener("pointerdown", unlockNotificationSound);
    document.addEventListener("keydown", unlockNotificationSound);
    const received = (event: Event) => {
      const { sender, room, roomId, eventId, message, silent } = (event as CustomEvent<MatrixMentionDetail>).detail;
      const id = `matrix:${roomId}:${eventId}`;
      if (storeRef.current.seen.includes(id)) return;
      if (!silent) void playNotificationSound();
      const previous = storeRef.current;
      const notice: Notice = { id, target: "matrix", version: "", message: `${sender}: ${message}`, source: room, roomId, eventId, unread: !openRef.current };
      commit({ ...previous, items: [notice, ...previous.items].slice(0, 20), seen: [...previous.seen, id].slice(-100) });
      if (!openRef.current && !silent) setToasts(queue => [...queue, notice]);
    };
    const read = (event: Event) => {
      const { roomId, eventIds, fullyRead } = (event as CustomEvent<MatrixReadDetail>).detail;
      const covered = (notice: Notice) => notice.target === "matrix" && notice.roomId === roomId && (fullyRead || Boolean(notice.eventId && eventIds.includes(notice.eventId)));
      const previous = storeRef.current;
      if (previous.items.some(notice => covered(notice) && notice.unread)) commit({ ...previous, items: previous.items.map(notice => covered(notice) ? { ...notice, unread: false } : notice) });
      setToasts(queue => queue.filter(notice => !covered(notice)));
    };
    window.addEventListener(MATRIX_MENTION_EVENT, received);
    window.addEventListener(MATRIX_READ_EVENT, read);
    return () => {
      window.removeEventListener(MATRIX_MENTION_EVENT, received);
      window.removeEventListener(MATRIX_READ_EVENT, read);
      document.removeEventListener("pointerdown", unlockNotificationSound);
      document.removeEventListener("keydown", unlockNotificationSound);
    };
  }, []);
  useEffect(() => {
    if (launcherUpdate) reconcile("launcher", launcherUpdate.available ? launcherUpdate.latest : null);
  }, [launcherUpdate]); // One response per startup or manual refresh.
  // Backends and runners are checked in their own panels (or on request), never at
  // start; their results arrive here through the events below.
  useEffect(() => {
    const updated = (event: Event) => {
      const { target, status } = (event as CustomEvent<{ target: UpdateTarget; status: BackendStatus }>).detail;
      reconcile(target, status.current && status.needs_update ? status.latest : null);
    };
    window.addEventListener("backendVersionChecked", updated);
    return () => window.removeEventListener("backendVersionChecked", updated);
  }, []);
  useEffect(() => {
    const checked = (event: Event) => {
      const { family, status } = (event as CustomEvent<{ family: ProtonFamily; status: BackendStatus }>).detail;
      protonResponses.current.add(family);
      reconcile(family, status?.current && status.needs_update && status.current !== status.latest ? status.latest : null);
    };
    const changed = (event: Event) => {
      const { family } = (event as CustomEvent<{ family: ProtonFamily }>).detail;
      protonResponses.current.add(family);
      reconcile(family, null);
    };
    window.addEventListener("protonVersionChecked", checked);
    window.addEventListener("protonInstallationChanged", changed);
    return () => { window.removeEventListener("protonVersionChecked", checked); window.removeEventListener("protonInstallationChanged", changed); };
  }, []);
  useEffect(() => {
    if (!import.meta.env.DEV || !("__PELI_UI_PREVIEW__" in window)) return;
    const simulate = () => {
      openRef.current = false; setOpen(false); setToasts([]);
      const version = `preview-${Date.now()}`;
      const notices: Notice[] = (["launcher", "rdna4", "rdna3", "ge-proton", "cachyos-proton"] as const).map(target => ({ id: `${target}:${version}`, target, version, unread: true }));
      commit({ items: [...notices, ...storeRef.current.items].slice(0, 20), seen: [...storeRef.current.seen, ...notices.map(n => n.id)].slice(-100) });
      setToasts(notices);
    };
    const simulateError = () => {
      openRef.current = false; setOpen(false); setToasts([]);
      notifyNexusError("Ferramenta não instalada: FNVpatch.exe. Instale o FNV 4GB Patcher na pasta do jogo para utilizar este atalho.");
    };
    window.addEventListener("previewSimulateNotifications", simulate);
    window.addEventListener("previewSimulateNexusError", simulateError);
    return () => {
      window.removeEventListener("previewSimulateNotifications", simulate);
      window.removeEventListener("previewSimulateNexusError", simulateError);
    };
  }, []);
  useEffect(() => {
    const content = contentRef.current;
    if (!content) return;
    const measure = () => setPanelHeight(content.getBoundingClientRect().height + 2);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(content);
    return () => observer.disconnect();
  }, []);
  useLayoutEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const measure = () => {
      const entries = Array.from(list.querySelectorAll<HTMLElement>(".notification-entry")).slice(0, 3);
      if (!entries.length) return;
      setListHeight(entries.reduce((height, entry, index) => height + entry.offsetHeight + (index < entries.length - 1 ? parseFloat(getComputedStyle(entry).marginBottom) || 0 : 0), 0));
      entries.forEach(entry => resize.observe(entry));
    };
    const resize = new ResizeObserver(measure);
    const children = new MutationObserver(measure);
    resize.observe(list);
    children.observe(list, { childList: true, subtree: true });
    measure();
    return () => { resize.disconnect(); children.disconnect(); };
  }, [store.items, language]);
  const toast = toasts[0];
  useLayoutEffect(() => {
    setToastHovered(toastRef.current?.matches(":hover") ?? false);
    setToastFocused(Boolean(toastRef.current && document.activeElement === toastRef.current));
  }, [toast?.id]);
  useEffect(() => {
    if (!toast) return;
    if (toastTime.current.id !== toast.id) toastTime.current = { id: toast.id, remaining: 3000 };
    if (toastHovered || toastFocused) return;
    const started = performance.now();
    const timer = window.setTimeout(() => setToasts(queue => queue.slice(1)), toastTime.current.remaining);
    return () => {
      window.clearTimeout(timer);
      toastTime.current.remaining = Math.max(0, toastTime.current.remaining - (performance.now() - started));
    };
  }, [toast?.id, toastHovered, toastFocused]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !panelRef.current?.contains(event.target) && !bellRef.current?.contains(event.target)) { openRef.current = false; setOpen(false); }
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && openRef.current) {
        event.preventDefault(); event.stopImmediatePropagation();
        openRef.current = false; setOpen(false); bellRef.current?.focus();
      }
    };
    window.addEventListener("keydown", escape, true);
    return () => window.removeEventListener("keydown", escape, true);
  }, []);
  const title = (n: Notice) => n.target === "matrix"
    ? tx("ui-notification-center", "matrix_you_were_mentioned_in", {"n_source_peligames": n.source || "PeliGames"})
    : n.target === "info"
    ? n.source || "PeliGames"
    : n.target === "error"
    ? `${n.source || "PeliGames"} — ${tx("ui-notification-center", "error")}`
    : n.target === "nexus"
    ? tx("ui-notification-center", "nexus_mods_action_failed")
    : n.target === "launcher"
    ? tx("ui-notification-center", "peligames_update_available")
    : n.target === "ge-proton" || n.target === "cachyos-proton"
    ? tx("ui-notification-center", "update_available", {"n_target_ge_proton_proton_ge_latest": n.target === "ge-proton" ? "Proton-GE Latest" : "Proton-CachyOS Latest"})
    : tx("ui-notification-center", "rx_backend_update_available", {"n_target_rdna4_9000_7000": n.target === "rdna4" ? "9000" : "7000"});
  const close = () => { openRef.current = false; setOpen(false); };
  const navigate = (notice: Notice) => {
    if (navigationBusy) return;
    commit({ ...storeRef.current, items: storeRef.current.items.map(n => n.id === notice.id ? { ...n, unread: false } : n) });
    setToasts(queue => queue.filter(n => n.id !== notice.id));
    close(); if (notice.action === "nexus-account") onNexusAccount(); else if (notice.action === "mangohud-install") openMangoHudInstallation(); else if (notice.action === "frame-mako") openFrameGenerationPreferences("mako"); else if (notice.action === "frame-afmf") openFrameGenerationPreferences("afmf"); else if (notice.target === "matrix") window.dispatchEvent(new Event(OPEN_MATRIX_CHAT_EVENT)); else if (notice.target !== "nexus" && notice.target !== "error" && notice.target !== "info") onNavigate(notice.target);
  };
  return <>
    <button ref={bellRef} type="button" data-tauri-drag-region="false" className="btn-titlebar notification-bell" title={tx("ui-notification-center", "notifications")} aria-label={tx("ui-notification-center", "notifications")} aria-expanded={open} aria-controls="update-notifications" onClick={() => {
      openRef.current = !open; setOpen(!open);
      if (!open) { setToasts([]); commit({ ...storeRef.current, items: storeRef.current.items.map(n => ({ ...n, unread: false })) }); }
    }}><Bell/>{store.items.some(n => n.unread) && <span className="launcher-update-dot"/>}</button>
    {createPortal(<>
      <div className="notification-panel-viewport">
      <motion.aside ref={panelRef} initial={false} animate={{ height: panelHeight ?? "auto" }} transition={{ height: { duration: reducedMotion ? .08 : .26, ease: [.4, 0, .2, 1] } }} id="update-notifications" className={`notification-panel menu-glass-panel${open ? " is-open" : ""}`} aria-label={tx("ui-notification-center", "notifications")} inert={!open} aria-hidden={!open} onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); close(); bellRef.current?.focus(); } }}>
        <div ref={contentRef} className="notification-panel-content">
        <header><h2><Bell/>{tx("ui-notification-center", "notifications")}</h2></header>
        <div ref={listRef} className="notification-list" style={{ maxHeight: store.items.length > 3 || !store.items.length ? listHeight : undefined }}><AnimatePresence initial={false} mode="wait">{store.items.length ? <motion.div key="notices" initial="visible" animate="visible" exit="exit" variants={{ visible: {}, exit: { transition: { staggerChildren: reducedMotion ? 0 : .045, when: "afterChildren" } } }}>{store.items.map(n => <motion.button variants={{ visible: { x: 0, opacity: 1 }, exit: { x: reducedMotion ? 0 : "110%", opacity: 0 } }} transition={{ duration: reducedMotion ? .08 : .2, ease: "easeIn" }} type="button" className="notification-entry" disabled={navigationBusy} key={n.id} onClick={() => navigate(n)}><MenuIcon name={n.target === "nexus" || n.target === "error" || n.target === "info" ? "info" : n.target === "matrix" ? "chainLinked" : "download"}/><span><strong>{title(n)}</strong><small>{n.target === "nexus" || n.target === "error" || n.target === "info" || n.target === "matrix" ? n.message : `${tx("ui-notification-center", "version")} ${n.version}`}</small>{n.target !== "nexus" && n.target !== "error" && n.target !== "info" && <small className="notification-link">{n.target === "matrix" ? tx("ui-notification-center", "open_chat") : tx("ui-notification-center", "open_updater")} →</small>}</span></motion.button>)}</motion.div> : <motion.p key="empty" initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: .12 }}>{tx("ui-notification-center", "no_notifications")}</motion.p>}</AnimatePresence></div>
        <footer><button className="btn-primary" type="button" disabled={!store.items.length} onClick={() => { commit({ ...storeRef.current, items: [] }); setToasts([]); }}><MenuIcon name="trash"/>{tx("ui-notification-center", "clear_notifications")}</button></footer>
        </div>
      </motion.aside>
      </div>
      <div className="notification-toast-region" aria-live="polite" aria-atomic="true">{toast && <button ref={toastRef} key={toast.id} className="notification-toast menu-glass-panel" style={{ animationPlayState: toastHovered || toastFocused ? "paused" : "running" }} onMouseEnter={() => setToastHovered(true)} onMouseLeave={() => setToastHovered(false)} onFocus={() => setToastFocused(true)} onBlur={() => setToastFocused(false)} disabled={navigationBusy} type="button" onClick={() => navigate(toast)}><Bell/><span><strong>{title(toast)}</strong><small>{toast.target === "nexus" || toast.target === "error" || toast.target === "info" || toast.target === "matrix" ? toast.message : `${tx("ui-notification-center", "version")} ${toast.version}`}</small></span></button>}</div>
    </>, document.body)}
  </>;
}

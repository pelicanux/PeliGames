import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "framer-motion";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";
import type { LauncherUpdateInfo } from "../services/launcherUpdates";
import { checkRunner, type ProtonFamily } from "../services/protonService";
import "./notifications.css";
import { LAUNCHER_ERROR_EVENT, type LauncherErrorDetail } from "../services/notifications";
import { MATRIX_MENTION_EVENT, OPEN_MATRIX_CHAT_EVENT, type MatrixMentionDetail } from "../services/matrixChat";

export type UpdateTarget = "launcher" | "rdna4" | "rdna3" | ProtonFamily;
type Notice = { id: string; target: UpdateTarget | "nexus" | "error" | "matrix"; version: string; message?: string; source?: string; unread: boolean };
type Store = { items: Notice[]; seen: string[] };
type BackendStatus = { latest: string; current: string | null; needs_update: boolean };
const storageKey = "peligames_update_notifications_v1";
function readStore(): Store {
  try {
    const value = JSON.parse(localStorage.getItem(storageKey) || "null");
    if (Array.isArray(value?.items) && Array.isArray(value?.seen)) return {
      items: value.items.filter((n: Notice) => n && ["launcher", "rdna3", "rdna4", "ge-proton", "cachyos-proton", "nexus", "error", "matrix"].includes(n.target) && (!["nexus", "error", "matrix"].includes(n.target) || typeof n.message === "string") && typeof n.id === "string" && typeof n.version === "string").slice(0, 20),
      seen: value.seen.filter((id: unknown) => typeof id === "string").slice(-100),
    };
  } catch { /* Storage may be unavailable; notifications still work in memory. */ }
  return { items: [], seen: [] };
}
// StrictMode remounts share the asynchronous startup checks.
let componentChecks: Promise<PromiseSettledResult<{ target: UpdateTarget; status: BackendStatus }>[]> | undefined;
function checkComponents() {
  return componentChecks ??= Promise.allSettled([
    ...(["rdna4", "rdna3"] as const).map(async target => ({ target, status: await invoke<BackendStatus>("check_backend_version", { gpuArch: target }) })),
    ...(["ge-proton", "cachyos-proton"] as const).map(async target => ({ target, status: await checkRunner(target) })),
  ]);
}
const Bell = () => <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4"/></svg>;

export function NotificationCenter({ launcherUpdate, onNavigate, nexusError = "", navigationBusy = false }: { nexusError?: string; navigationBusy?: boolean; launcherUpdate: LauncherUpdateInfo | null; onNavigate: (target: UpdateTarget) => void }) {
  const { language } = useI18n();
  const en = language === "en";
  const reducedMotion = useReducedMotion();
  const [store, setStore] = useState(readStore);
  const storeRef = useRef(store);
  const protonResponses = useRef(new Set<ProtonFamily>());
  const [open, setOpen] = useState(false);
  const openRef = useRef(false);
  const [toasts, setToasts] = useState<Notice[]>([]);
  const [toastHovered, setToastHovered] = useState(false);
  const [toastFocused, setToastFocused] = useState(false);
  const toastTime = useRef({ id: "", remaining: 6000 });
  const panelRef = useRef<HTMLElement>(null);
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
  const notifyNexusError = (message: string) => {
    if (!message.trim()) return;
    const previous = storeRef.current;
    const sameMessage = (n: Notice) => n.target === "nexus" && n.message === message;
    const notice: Notice = { id: `nexus:${crypto.randomUUID()}`, target: "nexus", version: "", message, unread: !openRef.current };
    commit({ ...previous, items: [notice, ...previous.items.filter(n => !sameMessage(n))].slice(0, 20) });
    if (!openRef.current) setToasts(queue => [...queue.filter(n => !sameMessage(n)), notice]);
  };
  useEffect(() => { notifyNexusError(nexusError); }, [nexusError]);
  // Any launcher error (notifyError): kept like the Nexus ones, one entry per message.
  useEffect(() => {
    const received = (event: Event) => {
      const { source, message } = (event as CustomEvent<LauncherErrorDetail>).detail;
      if (!message?.trim()) return;
      const previous = storeRef.current;
      const sameMessage = (n: Notice) => n.target === "error" && n.message === message && n.source === source;
      const notice: Notice = { id: `error:${crypto.randomUUID()}`, target: "error", version: "", message, source, unread: !openRef.current };
      commit({ ...previous, items: [notice, ...previous.items.filter(n => !sameMessage(n))].slice(0, 20) });
      if (!openRef.current) setToasts(queue => [...queue.filter(n => !sameMessage(n)), notice]);
    };
    window.addEventListener(LAUNCHER_ERROR_EVENT, received);
    return () => window.removeEventListener(LAUNCHER_ERROR_EVENT, received);
  }, []);
  // Someone mentioned the user in the Matrix chat: one entry per message, opens the chat.
  useEffect(() => {
    const received = (event: Event) => {
      const { sender, room, message } = (event as CustomEvent<MatrixMentionDetail>).detail;
      const previous = storeRef.current;
      const notice: Notice = { id: `matrix:${crypto.randomUUID()}`, target: "matrix", version: "", message: `${sender}: ${message}`, source: room, unread: !openRef.current };
      commit({ ...previous, items: [notice, ...previous.items].slice(0, 20) });
      if (!openRef.current) setToasts(queue => [...queue, notice]);
    };
    window.addEventListener(MATRIX_MENTION_EVENT, received);
    return () => window.removeEventListener(MATRIX_MENTION_EVENT, received);
  }, []);
  useEffect(() => {
    if (launcherUpdate) reconcile("launcher", launcherUpdate.available ? launcherUpdate.latest : null);
  }, [launcherUpdate]); // One response per startup or manual refresh.
  useEffect(() => {
    let active = true;
    void checkComponents().then(results => {
      if (!active) return;
      for (const result of results) {
        if (result.status === "fulfilled") {
          const { target, status } = result.value;
          if (target === "ge-proton" || target === "cachyos-proton") {
            if (!protonResponses.current.has(target)) reconcile(target, status?.current && status.needs_update && status.current !== status.latest ? status.latest : null);
          } else {
            // First installations do not count as updates.
            reconcile(target, status?.current && status.needs_update ? status.latest : null);
          }
        } else console.debug("Update notification check unavailable", result.reason);
      }
    });
    return () => { active = false; };
  }, []);
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
  useEffect(() => {
    if (!toast) return;
    if (toastTime.current.id !== toast.id) toastTime.current = { id: toast.id, remaining: 6000 };
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
    ? en ? `Matrix — you were mentioned in #${n.source || "PeliGames"}` : `Matrix — você foi mencionado em #${n.source || "PeliGames"}`
    : n.target === "error"
    ? `${n.source || "PeliGames"} — ${en ? "error" : "erro"}`
    : n.target === "nexus"
    ? en ? "Nexus Mods — action failed" : "Nexus Mods — falha na ação"
    : n.target === "launcher"
    ? en ? "PeliGames update available" : "Atualização do PeliGames disponível"
    : n.target === "ge-proton" || n.target === "cachyos-proton"
    ? en ? `${n.target === "ge-proton" ? "Proton-GE Latest" : "Proton-CachyOS Latest"} update available` : `Atualização do ${n.target === "ge-proton" ? "Proton-GE Latest" : "Proton-CachyOS Latest"} disponível`
    : en ? `RX ${n.target === "rdna4" ? "9000" : "7000"} backend update available` : `Atualização do backend RX ${n.target === "rdna4" ? "9000" : "7000"} disponível`;
  const close = () => { openRef.current = false; setOpen(false); };
  const navigate = (notice: Notice) => {
    if (navigationBusy) return;
    commit({ ...storeRef.current, items: storeRef.current.items.map(n => n.id === notice.id ? { ...n, unread: false } : n) });
    setToasts(queue => queue.filter(n => n.id !== notice.id));
    close(); if (notice.target === "matrix") window.dispatchEvent(new Event(OPEN_MATRIX_CHAT_EVENT)); else if (notice.target !== "nexus" && notice.target !== "error") onNavigate(notice.target);
  };
  return <>
    <button ref={bellRef} type="button" data-tauri-drag-region="false" className="btn-titlebar notification-bell" title={en ? "Notifications" : "Notificações"} aria-label={en ? "Notifications" : "Notificações"} aria-expanded={open} aria-controls="update-notifications" onClick={() => {
      openRef.current = !open; setOpen(!open);
      if (!open) { setToasts([]); commit({ ...storeRef.current, items: storeRef.current.items.map(n => ({ ...n, unread: false })) }); }
    }}><Bell/>{store.items.some(n => n.unread) && <span className="launcher-update-dot"/>}</button>
    {createPortal(<>
      <div className="notification-panel-viewport">
      <motion.aside ref={panelRef} initial={false} animate={{ height: panelHeight ?? "auto" }} transition={{ height: { duration: reducedMotion ? .08 : .26, ease: [.4, 0, .2, 1] } }} id="update-notifications" className={`notification-panel menu-glass-panel${open ? " is-open" : ""}`} aria-label={en ? "Notifications" : "Notificações"} inert={!open} aria-hidden={!open} onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); close(); bellRef.current?.focus(); } }}>
        <div ref={contentRef} className="notification-panel-content">
        <header><h2><Bell/>{en ? "Notifications" : "Notificações"}</h2></header>
        <div ref={listRef} className="notification-list" style={{ maxHeight: store.items.length > 3 || !store.items.length ? listHeight : undefined }}><AnimatePresence initial={false} mode="wait">{store.items.length ? <motion.div key="notices" initial="visible" animate="visible" exit="exit" variants={{ visible: {}, exit: { transition: { staggerChildren: reducedMotion ? 0 : .045, when: "afterChildren" } } }}>{store.items.map(n => <motion.button variants={{ visible: { x: 0, opacity: 1 }, exit: { x: reducedMotion ? 0 : "110%", opacity: 0 } }} transition={{ duration: reducedMotion ? .08 : .2, ease: "easeIn" }} type="button" className="notification-entry" disabled={navigationBusy} key={n.id} onClick={() => navigate(n)}><MenuIcon name={n.target === "nexus" || n.target === "error" ? "info" : n.target === "matrix" ? "chainLinked" : "download"}/><span><strong>{title(n)}</strong><small>{n.target === "nexus" || n.target === "error" || n.target === "matrix" ? n.message : `${en ? "Version" : "Versão"} ${n.version}`}</small>{n.target !== "nexus" && n.target !== "error" && <small className="notification-link">{n.target === "matrix" ? en ? "Open chat" : "Abrir chat" : en ? "Open updater" : "Abrir atualização"} →</small>}</span></motion.button>)}</motion.div> : <motion.p key="empty" initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: .12 }}>{en ? "No notifications." : "Nenhuma notificação."}</motion.p>}</AnimatePresence></div>
        <footer><button className="btn-primary" type="button" disabled={!store.items.length} onClick={() => { commit({ ...storeRef.current, items: [] }); setToasts([]); }}><MenuIcon name="trash"/>{en ? "Clear notifications" : "Limpar notificações"}</button></footer>
        </div>
      </motion.aside>
      </div>
      <div className="notification-toast-region" aria-live="polite" aria-atomic="true">{toast && <button key={toast.id} className="notification-toast menu-glass-panel" style={{ animationPlayState: toastHovered || toastFocused ? "paused" : "running" }} onMouseEnter={() => setToastHovered(true)} onMouseLeave={() => setToastHovered(false)} onFocus={() => setToastFocused(true)} onBlur={() => setToastFocused(false)} disabled={navigationBusy} type="button" onClick={() => navigate(toast)}><Bell/><span><strong>{title(toast)}</strong><small>{toast.target === "nexus" || toast.target === "error" || toast.target === "matrix" ? toast.message : `${en ? "Version" : "Versão"} ${toast.version}`}</small></span></button>}</div>
    </>, document.body)}
  </>;
}

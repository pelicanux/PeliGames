import { useEffect } from "react";
import "./FloatingScrollbars.css";

type Scrollbar = { track: HTMLDivElement; thumb: HTMLDivElement; timer: number; dragging: boolean; hovering: boolean };

/** Overlay thumbs leave the native scroll container and its rounded edges intact. */
export function FloatingScrollbars() {
  useEffect(() => {
    const layer = document.createElement("div");
    layer.className = "floating-scroll-layer";
    layer.setAttribute("aria-hidden", "true");
    document.body.append(layer);
    const bars = new Map<HTMLElement, Scrollbar>();
    let frame = 0;

    const reveal = (bar: Scrollbar) => {
      bar.track.classList.add("is-active");
      window.clearTimeout(bar.timer);
      bar.timer = window.setTimeout(() => {
        if (!bar.dragging && !bar.hovering) bar.track.classList.remove("is-active");
      }, 1000);
    };
    const activeModal = () => Array.from(document.querySelectorAll<HTMLElement>(".modal-overlay, .preferences-overlay"))
      .filter(overlay => overlay.getClientRects().length > 0)
      .sort((a, b) => (Number.parseInt(getComputedStyle(a).zIndex, 10) || 0) - (Number.parseInt(getComputedStyle(b).zIndex, 10) || 0)).slice(-1)[0];
    const position = (host: HTMLElement, bar: Scrollbar, modal?: HTMLElement) => {
      const rect = host.getBoundingClientRect();
      const headerInset = host.classList.contains("modal-scroll-body") && host.parentElement?.classList.contains("has-dialog-titlebar") ? 72 : 12;
      let top = Math.max(12, rect.top + headerInset);
      let bottom = Math.min(window.innerHeight - 12, rect.bottom - 12);
      let right = rect.right;
      for (let parent = host.parentElement; parent; parent = parent.parentElement) {
        if (!/^(auto|scroll|hidden|clip)$/.test(getComputedStyle(parent).overflowY)) continue;
        const bounds = parent.getBoundingClientRect();
        top = Math.max(top, bounds.top);
        bottom = Math.min(bottom, bounds.bottom);
        right = Math.min(right, bounds.right);
      }
      const height = Math.max(0, bottom - top);
      // Panels can place the overlay in their existing outer padding, keeping content margins equal.
      const outside = host.dataset.scrollbarOutside === "true";
      const left = Math.min(window.innerWidth - 10, right + (outside ? 3 : -13));
      bar.track.style.cssText = `top:${top}px;left:${left}px;height:${height}px`;
      bar.track.hidden = Boolean(modal && !modal.contains(host)) || !host.getClientRects().length || height < 28 || rect.width === 0 || host.scrollHeight <= host.clientHeight;
      const thumbHeight = Math.min(height, Math.max(28, height * host.clientHeight / host.scrollHeight));
      bar.thumb.style.height = `${thumbHeight}px`;
      bar.thumb.style.transform = `translateY(${(height - thumbHeight) * host.scrollTop / Math.max(1, host.scrollHeight - host.clientHeight)}px)`;
    };
    const resize = new ResizeObserver(() => schedule());
    const scan = () => {
      frame = 0;
      for (const host of document.body.querySelectorAll<HTMLElement>("*")) {
        if (layer.contains(host) || bars.has(host) || host.scrollHeight <= host.clientHeight + 1) continue;
        if (!/^(auto|scroll)$/.test(getComputedStyle(host).overflowY)) continue;
        const track = document.createElement("div");
        track.className = "floating-scroll-track";
        const thumb = document.createElement("div");
        thumb.className = "floating-scroll-thumb";
        track.append(thumb);
        layer.append(track);
        const bar = { track, thumb, timer: 0, dragging: false, hovering: false };
        bars.set(host, bar);
        host.classList.add("floating-scroll-host");
        resize.observe(host);
        thumb.addEventListener("pointerdown", (event) => {
          event.preventDefault();
          bar.dragging = true;
          reveal(bar);
          thumb.setPointerCapture(event.pointerId);
          const initialY = event.clientY;
          const initialScroll = host.scrollTop;
          const travel = track.clientHeight - thumb.clientHeight;
          const move = (next: PointerEvent) => {
            host.scrollTop = initialScroll + (next.clientY - initialY) * (host.scrollHeight - host.clientHeight) / Math.max(1, travel);
          };
          const end = () => {
            bar.dragging = false;
            thumb.removeEventListener("pointermove", move);
            thumb.removeEventListener("pointerup", end);
            thumb.removeEventListener("pointercancel", end);
            reveal(bar);
          };
          thumb.addEventListener("pointermove", move);
          thumb.addEventListener("pointerup", end);
          thumb.addEventListener("pointercancel", end);
        });
      }
      const modal = activeModal();
      for (const [host, bar] of bars) {
        if (!host.isConnected) {
          window.clearTimeout(bar.timer);
          resize.unobserve(host);
          bar.track.remove();
          bars.delete(host);
        } else position(host, bar, modal);
      }
    };
    function schedule() { if (!frame) frame = requestAnimationFrame(scan); }
    const scroll = (event: Event) => {
      const bar = bars.get(event.target as HTMLElement);
      if (bar) reveal(bar);
      schedule();
    };
    const pointer = (event: PointerEvent) => {
      for (const bar of bars.values()) {
        const rect = bar.track.getBoundingClientRect();
        const near = !bar.track.hidden && event.clientX >= rect.left - 11 && event.clientX <= rect.right + 7 && event.clientY >= rect.top && event.clientY <= rect.bottom;
        const wasHovering = bar.hovering;
        bar.hovering = near;
        if (near || wasHovering) reveal(bar);
      }
    };
    const leave = () => {
      for (const bar of bars.values()) {
        if (bar.hovering) { bar.hovering = false; reveal(bar); }
      }
    };
    const mutations = new MutationObserver((changes) => {
      if (changes.some(change => !layer.contains(change.target))) schedule();
    });
    mutations.observe(document.body, { childList: true, subtree: true, characterData: true });
    document.addEventListener("scroll", scroll, true);
    document.addEventListener("pointermove", pointer, { passive: true });
    document.addEventListener("pointerleave", leave);
    window.addEventListener("resize", schedule);
    scan();
    return () => {
      cancelAnimationFrame(frame);
      mutations.disconnect();
      resize.disconnect();
      document.removeEventListener("scroll", scroll, true);
      document.removeEventListener("pointermove", pointer);
      document.removeEventListener("pointerleave", leave);
      window.removeEventListener("resize", schedule);
      for (const [host, bar] of bars) {
        window.clearTimeout(bar.timer);
        host.classList.remove("floating-scroll-host");
      }
      layer.remove();
    };
  }, []);
  return null;
}

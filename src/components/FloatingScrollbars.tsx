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
    const position = (host: HTMLElement, bar: Scrollbar) => {
      const rect = host.getBoundingClientRect();
      const headerInset = host.classList.contains("modal-scroll-body") && host.parentElement?.classList.contains("has-dialog-titlebar") ? 72 : 12;
      const top = Math.max(12, rect.top + headerInset);
      const bottom = Math.min(window.innerHeight - 12, rect.bottom - 12);
      const height = Math.max(0, bottom - top);
      bar.track.style.cssText = `top:${top}px;left:${rect.right - 13}px;height:${height}px`;
      bar.track.hidden = height < 28 || rect.width === 0 || host.scrollHeight <= host.clientHeight;
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
      for (const [host, bar] of bars) {
        if (!host.isConnected) {
          window.clearTimeout(bar.timer);
          resize.unobserve(host);
          bar.track.remove();
          bars.delete(host);
        } else position(host, bar);
      }
    };
    function schedule() { if (!frame) frame = requestAnimationFrame(scan); }
    const scroll = (event: Event) => {
      const bar = bars.get(event.target as HTMLElement);
      if (bar) reveal(bar);
      schedule();
    };
    const pointer = (event: PointerEvent) => {
      for (const [host, bar] of bars) {
        const rect = host.getBoundingClientRect();
        const near = event.clientX >= rect.right - 24 && event.clientX <= rect.right && event.clientY >= rect.top && event.clientY <= rect.bottom;
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

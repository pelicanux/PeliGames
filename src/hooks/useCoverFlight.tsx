import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

type Flight = { source: DOMRect; portrait: HTMLElement; target?: HTMLElement; returning?: boolean; onComplete?: () => void; landed?: boolean; landing?: HTMLElement | null };

// Escape panel stacking contexts and scroll clipping, following the destination
// while the details slide into place.
export function useCoverFlight(disabled: boolean) {
  const destinationRef = useRef<HTMLDivElement>(null);
  const layerRef = useRef<HTMLDivElement>(null);
  const [flight, setFlight] = useState<Flight | null>(null);
  const start = (source?: HTMLElement) => {
    if (disabled || !source?.isConnected || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const bounds = source.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const portrait = source.cloneNode(true) as HTMLElement;
    // Do not carry Framer's projection transform into the floating copy.
    portrait.style.transform = "none";
    portrait.style.willChange = "auto";
    Object.assign(portrait.style, { width: "100%", height: "100%", margin: "0", border: "0", boxShadow: "none", visibility: "visible" });
    setFlight({ source: bounds, portrait });
  };
  const returnTo = (target: HTMLElement | undefined, onComplete: () => void) => {
    const source = destinationRef.current;
    if (disabled || !source?.isConnected || !target?.isConnected || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return false;
    // A rapid collapse starts at the currently visible floating portrait.
    const bounds = (flight ? layerRef.current : source)?.getBoundingClientRect();
    if (!bounds?.width || !bounds.height) return false;
    // Return the library's own artwork, without the details cover's glare or
    // platform overlay. Otherwise removing the copy visibly changes the image.
    const portrait = target.cloneNode(true) as HTMLElement;
    Object.assign(portrait.style, { transform: "none", willChange: "auto", width: "100%", height: "100%", margin: "0", border: "0", boxShadow: "none", visibility: "visible" });
    // The card the copy lands on: its grey copy becomes the full cover when it lands.
    setFlight({ source: bounds, portrait, target, returning: true, onComplete, landing: target.closest<HTMLElement>(".library-card-cell") });
    return true;
  };
  useLayoutEffect(() => {
    const layer = layerRef.current;
    if (!flight || !layer) return;
    if (disabled) { setFlight(null); flight.onComplete?.(); return; }
    // Landed: the real cover is already shown under the copy; the copy leaves two
    // frames later, once WebKitGTK has painted it, so no frame is without a cover.
    if (flight.landed) {
      // Wait until the images where it landed are decoded (the card that just
      // turned from grey to colour, or the details cover), at most 400 ms.
      let frame = 0, active = true, fadeTimer = 0, decodeTimer = 0;
      const host = flight.returning ? flight.landing : destinationRef.current;
      // The away card normally restores its colour over 250 ms. Finish that
      // underneath the copy before fading it, or the handoff exposes a dim card.
      if (flight.returning) host?.setAttribute("data-cover-landing", "");
      frame = requestAnimationFrame(() => {
        const images = Array.from(host?.querySelectorAll("img") ?? []).filter(image => image.isConnected && image.getAttribute("src"));
        const decoded = Promise.all(images.map(image => image.decode().catch(() => {})));
        void Promise.race([decoded, new Promise(resolve => { decodeTimer = window.setTimeout(resolve, 400); })]).then(() => {
          clearTimeout(decodeTimer);
          // Two frames: the cover shows once its decoded state is rendered.
          if (active) frame = requestAnimationFrame(() => { if (active) frame = requestAnimationFrame(() => {
            if (!active) return;
            // Crossfade onto the already painted card rather than hard-cutting
            // between two differently composited images in WebKitGTK.
            layer.style.transition = "opacity 100ms ease-out";
            layer.style.opacity = "0";
            fadeTimer = window.setTimeout(() => { if (active) setFlight(null); }, 100);
          }); });
        });
      });
      return () => { active = false; cancelAnimationFrame(frame); clearTimeout(fadeTimer); clearTimeout(decodeTimer); if (flight.returning) host?.removeAttribute("data-cover-landing"); };
    }
    layer.replaceChildren(flight.portrait);
    layer.style.transition = "none";
    layer.style.opacity = "1";
    layer.style.boxShadow = "";
    const { source } = flight;
    layer.style.width = `${source.width}px`;
    layer.style.height = `${source.height}px`;
    layer.style.left = "0";
    layer.style.top = "0";
    layer.style.transform = `translate3d(${source.left}px, ${source.top}px, 0)`;
    let frame = 0;
    const started = performance.now();
    const tick = (now: number) => {
      const destination = flight.target ?? destinationRef.current;
      if (!destination?.isConnected) { setFlight(null); flight.onComplete?.(); return; }
      const target = destination.getBoundingClientRect();
      const progress = Math.min((now - started) / 360, 1);
      // The nearby overlay needs a short flight, with a smooth takeoff and landing.
      const eased = (1 - Math.cos(Math.PI * progress)) / 2;
      const mix = (from: number, to: number) => from + (to - from) * eased;
      // Keep subpixel movement on the compositor, including while the library
      // scrolls and the details expand. Rounding makes the slow landing step.
      layer.style.transform = `translate3d(${mix(source.left, target.left)}px, ${mix(source.top, target.top)}px, 0)`;
      layer.style.width = `${mix(source.width, target.width)}px`;
      layer.style.height = `${mix(source.height, target.height)}px`;
      if (flight.returning) {
        // Let the flight shadow settle before handing over to the flat card.
        layer.style.boxShadow = `0 24px 50px rgba(0,0,0,${.55 * (1 - eased)}), 0 0 0 1px rgba(255,255,255,${.12 * (1 - eased)})`;
      }
      if (progress < 1) frame = requestAnimationFrame(tick);
      else { setFlight({ ...flight, landed: true }); flight.onComplete?.(); }
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [flight, disabled]);
  return {
    start, returnTo, destinationRef, flying: flight !== null && !flight.landed, returning: Boolean(flight?.returning && !flight.landed),
    overlay: flight ? createPortal(<div ref={layerRef} className={`cover-flight-layer cover-flight-layer--modern`} aria-hidden="true" />, document.body) : null,
  };
}

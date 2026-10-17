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
      let frame = 0, trackingFrame = 0, active = true, fadeTimer = 0, decodeTimer = 0;
      const host = flight.returning ? flight.landing : destinationRef.current;
      // The handoff still has a floating copy for decoding/crossfade. Keep it
      // attached to the live card if the library scrolls during those frames.
      const align = () => {
        trackingFrame = 0;
        const target = flight.returning
          ? flight.landing?.querySelector<HTMLElement>(".selected-cover-copy, .library-cover-thumbnail")
          : destinationRef.current;
        if (!active || !target?.isConnected) return;
        const rect = target.getBoundingClientRect();
        if (!rect.width || !rect.height) return;
        layer.style.transform = `translate3d(${rect.left}px, ${rect.top}px, 0) scale(${rect.width / layer.offsetWidth}, ${rect.height / layer.offsetHeight})`;
      };
      const track = () => { if (!trackingFrame) trackingFrame = requestAnimationFrame(align); };
      const observer = new ResizeObserver(track);
      if (host) observer.observe(host);
      window.addEventListener("scroll", track, true);
      window.addEventListener("resize", track);
      align();
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
      return () => {
        active = false; cancelAnimationFrame(frame); cancelAnimationFrame(trackingFrame);
        clearTimeout(fadeTimer); clearTimeout(decodeTimer); observer.disconnect();
        window.removeEventListener("scroll", track, true); window.removeEventListener("resize", track);
        if (flight.returning) host?.removeAttribute("data-cover-landing");
      };
    }
    const destination = flight.target ?? destinationRef.current;
    if (!destination?.isConnected) { setFlight(null); flight.onComplete?.(); return; }
    const { source } = flight;
    const target = destination.getBoundingClientRect();
    if (!target.width || !target.height) { setFlight(null); flight.onComplete?.(); return; }
    // Rasterize once at the larger size, then move/scale that surface entirely
    // on the compositor. Changing width/height every frame repaints the image.
    const width = Math.max(source.width, target.width);
    const height = Math.max(source.height, target.height);
    const portrait = document.createElement("div");
    portrait.className = "cover-flight-portrait";
    portrait.append(flight.portrait);
    const shadow = document.createElement("div");
    shadow.className = "cover-flight-shadow";
    layer.replaceChildren(shadow, portrait);
    Object.assign(layer.style, { transition: "none", opacity: "1", width: `${width}px`, height: `${height}px`, left: "0", top: "0" });
    const transform = (rect: DOMRect) => `translate3d(${rect.left}px, ${rect.top}px, 0) scale(${rect.width / width}, ${rect.height / height})`;
    layer.style.transform = transform(source);
    let active = true, completed = false;
    let movement: Animation | undefined;
    let trackingFrame = 0;
    const duration = 310;
    let lastTarget = target;
    const finish = () => {
      if (!active || completed) return;
      completed = true;
      // A final scroll event can be queued in the same frame as animation end.
      // Land at the live destination, never at a stale endpoint.
      if (destination.isConnected) {
        const current = destination.getBoundingClientRect();
        if (current.width && current.height) lastTarget = current;
      }
      layer.style.transform = transform(lastTarget);
      if (flight.returning) shadow.style.opacity = "0";
      movement?.cancel();
      setFlight({ ...flight, landed: true });
      flight.onComplete?.();
    };
    movement = layer.animate([{ transform: transform(source) }, { transform: transform(target) }], {
      duration, easing: "cubic-bezier(.25, .1, .25, 1)", fill: "forwards",
    });
    void movement.finished.then(finish).catch(() => {});
    // Fade a separately composited shadow rather than repainting box-shadow.
    const shadowFade = flight.returning ? shadow.animate([{ opacity: 1 }, { opacity: 0 }], {
      duration, easing: "ease-out", fill: "forwards",
    }) : undefined;
    const retarget = () => {
      if (!active || completed) return;
      if (!destination.isConnected) { finish(); return; }
      const next = destination.getBoundingClientRect();
      if (!next.width || !next.height) return;
      if ((["left", "top", "width", "height"] as const).every(key => Math.abs(next[key] - lastTarget[key]) < .25)) return;
      lastTarget = next;
      // Update only the endpoint. Restarting an ease-in on every scroll event
      // repeatedly brakes the portrait and makes it chase the moving card.
      // Keeping the same animation clock also keeps its scale/velocity curve.
      (movement?.effect as KeyframeEffect | null)?.setKeyframes([
        { transform: transform(source) }, { transform: transform(next) },
      ]);
    };
    const scheduleRetarget = () => {
      if (!trackingFrame) trackingFrame = requestAnimationFrame(() => { trackingFrame = 0; retarget(); });
    };
    // Only remeasure when the destination actually changes, never every frame.
    const observer = new ResizeObserver(scheduleRetarget);
    observer.observe(destination);
    if (destination.parentElement) observer.observe(destination.parentElement);
    window.addEventListener("resize", scheduleRetarget);
    window.addEventListener("scroll", scheduleRetarget, true);
    return () => {
      active = false;
      cancelAnimationFrame(trackingFrame);
      movement?.cancel(); shadowFade?.cancel(); observer.disconnect();
      window.removeEventListener("resize", scheduleRetarget);
      window.removeEventListener("scroll", scheduleRetarget, true);
    };

  }, [flight, disabled]);
  return {
    start, returnTo, destinationRef, flying: flight !== null && !flight.landed, returning: Boolean(flight?.returning && !flight.landed),
    overlay: flight ? createPortal(<div ref={layerRef} className={`cover-flight-layer cover-flight-layer--modern`} aria-hidden="true" />, document.body) : null,
  };
}

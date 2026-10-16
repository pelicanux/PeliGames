import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

type Flight = { source: DOMRect; portrait: HTMLElement; target?: HTMLElement; returning?: boolean; onComplete?: () => void };

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
    const portrait = source.cloneNode(true) as HTMLElement;
    Object.assign(portrait.style, { transform: "none", willChange: "auto", width: "100%", height: "100%", margin: "0", border: "0", boxShadow: "none", visibility: "visible" });
    setFlight({ source: bounds, portrait, target, returning: true, onComplete });
    return true;
  };
  useLayoutEffect(() => {
    const layer = layerRef.current;
    if (!flight || !layer) return;
    if (disabled) { setFlight(null); flight.onComplete?.(); return; }
    layer.replaceChildren(flight.portrait);
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
      if (progress < 1) frame = requestAnimationFrame(tick);
      else { setFlight(null); flight.onComplete?.(); }
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [flight, disabled]);
  return {
    start, returnTo, destinationRef, flying: flight !== null, returning: Boolean(flight?.returning),
    overlay: flight ? createPortal(<div ref={layerRef} className={`cover-flight-layer cover-flight-layer--modern`} aria-hidden="true" />, document.body) : null,
  };
}

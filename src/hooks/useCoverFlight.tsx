import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

type Flight = { source: DOMRect; portrait: HTMLElement };

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
    setFlight({ source: bounds, portrait });
  };
  useLayoutEffect(() => {
    const layer = layerRef.current;
    if (!flight || !layer) return;
    if (disabled) { setFlight(null); return; }
    layer.replaceChildren(flight.portrait);
    const { source } = flight;
    layer.style.width = `${source.width}px`;
    layer.style.height = `${source.height}px`;
    layer.style.left = `${source.left}px`;
    layer.style.top = `${source.top}px`;
    let frame = 0;
    const started = performance.now();
    const tick = (now: number) => {
      const destination = destinationRef.current;
      if (!destination?.isConnected) { setFlight(null); return; }
      const target = destination.getBoundingClientRect();
      const progress = Math.min((now - started) / 520, 1);
      const eased = 1 - Math.pow(1 - progress, 3);
      const mix = (from: number, to: number) => from + (to - from) * eased;
      // Resize the image itself: scaling a composited thumbnail magnifies its
      // small raster and looks blurred until the original portrait replaces it.
      const pixels = (value: number) => `${Math.round(value * window.devicePixelRatio) / window.devicePixelRatio}px`;
      layer.style.left = pixels(mix(source.left, target.left));
      layer.style.top = pixels(mix(source.top, target.top));
      layer.style.width = pixels(mix(source.width, target.width));
      layer.style.height = pixels(mix(source.height, target.height));
      if (progress < 1) frame = requestAnimationFrame(tick);
      else setFlight(null);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [flight, disabled]);
  return {
    start, destinationRef, flying: flight !== null,
    overlay: flight ? createPortal(<div ref={layerRef} className="cover-flight-layer" aria-hidden="true" />, document.body) : null,
  };
}

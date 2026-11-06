import {useLayoutEffect, useRef} from "react";

/** Measure only layout changes; scrolling never updates toolbar position or React state. */
export function useLibraryHeader() {
  const viewport = useRef<HTMLDivElement>(null);
  const header = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const root = viewport.current, element = header.current;
    if (!root || !element) return;
    let previous = -1;
    const measure = () => {
      const height = element.getBoundingClientRect().height;
      if (height === previous) return;
      previous = height;
      root.style.setProperty("--library-header-height", `${height}px`);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => {observer.disconnect(); root.style.removeProperty("--library-header-height");};
  }, []);
  return {viewport, header};
}

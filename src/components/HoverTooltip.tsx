import { cloneElement, useId, useLayoutEffect, useRef, useState, type ReactElement, type HTMLAttributes } from "react";
import { createPortal } from "react-dom";

interface Props {
  text: string;
  children: ReactElement<HTMLAttributes<HTMLElement>>;
  anchorClassName?: string;
  tooltipClassName?: string;
}

/** Render outside the toolbar so its stacking context cannot clip the hint. */
export function HoverTooltip({ text, children, anchorClassName = "", tooltipClassName = "" }: Props) {
  const id = useId();
  const anchor = useRef<HTMLSpanElement>(null);
  const tooltip = useRef<HTMLDivElement>(null);
  const [hovered, setHovered] = useState(false);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);

  useLayoutEffect(() => {
    if (!hovered) return;
    const place = () => {
      if (!anchor.current || !tooltip.current) return;
      const bounds = anchor.current.getBoundingClientRect();
      const hint = tooltip.current.getBoundingClientRect();
      const margin = 8;
      const titlebarBottom = document.querySelector(".app-titlebar")?.getBoundingClientRect().bottom ?? margin;
      const minTop = titlebarBottom + margin;
      const above = bounds.top - hint.height - margin;
      const top = above >= minTop ? above : bounds.bottom + margin;
      setPosition({
        left: Math.max(margin, Math.min(bounds.right - hint.width, window.innerWidth - hint.width - margin)),
        top: Math.max(minTop, Math.min(top, window.innerHeight - hint.height - margin)),
      });
    };
    place();
    const observer = new ResizeObserver(place);
    if (tooltip.current) observer.observe(tooltip.current);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [hovered, text]);

  const close = () => { setHovered(false); setPosition(null); };
  return <span ref={anchor} className={`hover-tooltip-anchor ${anchorClassName}`}
    onMouseEnter={() => text.trim() && setHovered(true)} onMouseLeave={close} onPointerDown={close}>
    {cloneElement(children, { "aria-describedby": hovered ? id : undefined })}
    {hovered && text.trim() && createPortal(<div ref={tooltip} id={id} role="tooltip" className={`ui-hover-tooltip ${tooltipClassName}`}
      style={{ left: position?.left ?? 0, top: position?.top ?? 0, visibility: position ? "visible" : "hidden" }}>
      {text}
    </div>, document.body)}
  </span>;
}

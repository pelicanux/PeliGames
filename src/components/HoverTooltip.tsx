import { cloneElement, useId, useLayoutEffect, useRef, useState, type ReactElement, type HTMLAttributes } from "react";
import {TooltipBubble} from "./TooltipBubble";

interface Props {
  text: string;
  children: ReactElement<HTMLAttributes<HTMLElement>>;
  anchorClassName?: string;
  tooltipClassName?: string;
  showOnFocus?: boolean;
  align?: "left" | "right";
  hoverDelay?: number;
}

/** Render outside the toolbar so its stacking context cannot clip the hint. */
export function HoverTooltip({ text, children, anchorClassName = "", tooltipClassName = "", showOnFocus = true, align = "right", hoverDelay = 0 }: Props) {
  const id = useId();
  const anchor = useRef<HTMLSpanElement>(null);
  const suppressRestoredFocus = useRef(false);
  const hoverTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [hovered, setHovered] = useState(false);

  useLayoutEffect(() => {
    return () => { if (hoverTimer.current !== null) clearTimeout(hoverTimer.current); };
  }, [text, hoverDelay]);

  useLayoutEffect(() => {
    if (!text.trim()) { setHovered(false); }
  }, [text]);

  const cancelHover = () => { if (hoverTimer.current !== null) clearTimeout(hoverTimer.current); hoverTimer.current = null; };
  const close = () => { cancelHover(); setHovered(false); };
  const activate = () => { suppressRestoredFocus.current = true; close(); };
  useLayoutEffect(() => {
    if (!hovered) return;
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") {event.stopPropagation(); close();} };
    document.addEventListener("keydown", escape, true);
    window.addEventListener("blur", close);
    return () => { document.removeEventListener("keydown", escape, true); window.removeEventListener("blur", close); };
  }, [hovered]);

  return <span ref={anchor} className={`hover-tooltip-anchor ${anchorClassName}`}
    onMouseEnter={() => {
      suppressRestoredFocus.current = false;
      cancelHover();
      if (!text.trim()) return;
      if (hoverDelay <= 0) setHovered(true);
      else hoverTimer.current = setTimeout(() => { hoverTimer.current = null; setHovered(true); }, hoverDelay);
    }}
    onMouseLeave={close}
    onFocus={event => {
      // Closing a dialog restores focus to its trigger; that must not reopen its hint.
      const previous = event.relatedTarget;
      if (previous instanceof Element && !previous.closest('[role="dialog"], [aria-modal="true"]')) {
        suppressRestoredFocus.current = false;
      }
      if (!suppressRestoredFocus.current && showOnFocus && text.trim()) setHovered(true);
    }}
    onBlur={close} onPointerDown={activate} onClickCapture={activate}
    onKeyDownCapture={event => { if (event.key === "Escape" && hovered) {event.stopPropagation();close();} else if (event.key === "Enter" || event.key === " ") activate(); }}>
    {cloneElement(children, { "aria-describedby": hovered ? id : undefined })}
    {hovered && text.trim() && anchor.current && <TooltipBubble id={id} text={text} anchor={anchor.current} align={align} className={tooltipClassName}/>}
  </span>;
}

import { useLayoutEffect, useRef, useState } from "react";
import { HoverTooltip } from "../../components/HoverTooltip";

/** Fade only overflowing names; the full name stays available outside the row. */
export function NexusModTitle({ name }: { name: string }) {
  const title = useRef<HTMLElement>(null);
  const [overflow, setOverflow] = useState(false);
  useLayoutEffect(() => {
    const element = title.current;
    if (!element) return;
    const measure = () => setOverflow(element.scrollWidth > element.clientWidth + 1);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [name]);
  return <HoverTooltip text={name} hoverDelay={300} align="left" anchorClassName="nexus-mod-name-anchor">
    <strong ref={title} tabIndex={0} className={`nexus-mod-name${overflow ? " is-overflowing" : ""}`}>{name}</strong>
  </HoverTooltip>;
}

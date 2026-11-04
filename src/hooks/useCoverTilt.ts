import { useEffect, useRef, type MouseEvent } from "react";

export function useCoverTilt(performanceMode: boolean, gamePath?: string) {
  const detailHoverFrame = useRef<number | null>(null);
  const detailHoverBounds = useRef<{ card: HTMLDivElement; rect: DOMRect } | null>(null);
  const detailHoverPoint = useRef({ x: 0, y: 0 });
  useEffect(() => () => {
    if (detailHoverFrame.current !== null) cancelAnimationFrame(detailHoverFrame.current);
    detailHoverFrame.current = null;
    detailHoverBounds.current = null;
  }, [performanceMode, gamePath]);
  const handleCoverMouseMove = (e: MouseEvent<HTMLDivElement>) => {
    if (performanceMode) return;
    const card = e.currentTarget;
    if (detailHoverBounds.current?.card !== card) detailHoverBounds.current = { card, rect: card.getBoundingClientRect() };
    detailHoverPoint.current = { x: e.clientX, y: e.clientY };
    if (detailHoverFrame.current !== null) return;
    detailHoverFrame.current = requestAnimationFrame(() => {
      detailHoverFrame.current = null;
      const bounds = detailHoverBounds.current;
      if (!bounds || bounds.card !== card || !card.isConnected) return;
      const { rect } = bounds;
      if (!rect.width || !rect.height) return;
      const px = Math.max(0, Math.min(1, (detailHoverPoint.current.x - rect.left) / rect.width));
      const py = Math.max(0, Math.min(1, (detailHoverPoint.current.y - rect.top) / rect.height));
      card.style.transition = "transform .12s ease-out, box-shadow .25s ease";
      card.style.transform = `perspective(900px) rotateX(${(py * 2 - 1) * -6}deg) rotateY(${(px * 2 - 1) * 6}deg) scale3d(1.04, 1.04, 1.04)`;
      const glare = card.querySelector<HTMLElement>('.glare');
      if (glare) {
        glare.style.setProperty("--glare-x", `${px * 100}%`);
        glare.style.setProperty("--glare-y", `${py * 100}%`);
        glare.style.setProperty("--glare-sheen", `${120 - (px * 70 + py * 30) * 1.4}%`);
        glare.style.opacity = "1";
      }
    });
  };

  const handleCoverMouseLeave = (e: MouseEvent<HTMLDivElement>) => {
    if (detailHoverFrame.current !== null) cancelAnimationFrame(detailHoverFrame.current);
    detailHoverFrame.current = null;
    detailHoverBounds.current = null;
    const card = e.currentTarget;
    card.style.transition = "transform .45s cubic-bezier(.2, .8, .2, 1), box-shadow .35s ease";
    card.style.transform = "none";
    const glare = card.querySelector<HTMLElement>('.glare');
    if (glare) glare.style.opacity = "0";
  };

  return { onMove: handleCoverMouseMove, onLeave: handleCoverMouseLeave };
}

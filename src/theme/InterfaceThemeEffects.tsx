import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

/** Low-resolution digital rain; no DOM particle grid or per-character React renders. */
export function InterfaceThemeEffects({ active, transitioning }: { active: boolean; transitioning: boolean }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    if (!active) return;
    const canvas = canvasRef.current, context = canvas?.getContext("2d");
    if (!canvas || !context) return;
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    let timer: ReturnType<typeof setInterval> | undefined;
    let drops: number[] = [];
    const glyphs = "012345789アカサタナハマヤラワ<>/:{}";
    const draw = () => {
      context.fillStyle = "rgba(2, 9, 5, .12)";
      context.fillRect(0, 0, canvas.width, canvas.height);
      context.font = '14px "Jersey 10", monospace';
      drops.forEach((drop, index) => {
        context.fillStyle = index % 7 === 0 ? "#83bd91" : "#237a43";
        context.fillText(glyphs[Math.floor(Math.random() * glyphs.length)], index * 18, drop * 18);
        drops[index] = drop * 18 > canvas.height && Math.random() > .975 ? -Math.random() * 20 : drop + .45;
      });
    };
    const resize = () => {
      canvas.width = Math.min(960, innerWidth); canvas.height = Math.round(canvas.width * innerHeight / innerWidth);
      drops = Array.from({ length: Math.ceil(canvas.width / 18) }, () => Math.random() * canvas.height / 18);
      context.fillStyle = "#020905"; context.fillRect(0, 0, canvas.width, canvas.height);
      for (let i = 0; i < 24; i++) draw();
    };
    const pause = () => {
      clearInterval(timer);
      if (!document.hidden && !motion.matches && !document.body.classList.contains("performance-mode")) timer = setInterval(draw, 100);
    };
    const observer = new MutationObserver(pause);
    observer.observe(document.body, { attributes: true, attributeFilter: ["class"] });
    resize(); pause();
    window.addEventListener("resize", resize); document.addEventListener("visibilitychange", pause); motion.addEventListener("change", pause);
    return () => { clearInterval(timer); observer.disconnect(); window.removeEventListener("resize", resize); document.removeEventListener("visibilitychange", pause); motion.removeEventListener("change", pause); };
  }, [active]);
  return createPortal(<>
    {active && <><canvas ref={canvasRef} className="hackerman-rain" aria-hidden="true"/><div className="hackerman-crt" aria-hidden="true"/></>}
    {transitioning && <div className="theme-transition-status" role="status">{active ? "HACKERMAN" : "PELiGAMES"} / SYSTEM THEME LOADING_</div>}
  </>, document.body);
}

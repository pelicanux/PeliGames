import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import type { GamePanelMode } from "./GameModeSelector";

interface Props {
  mode: GamePanelMode;
  reducedMotion: boolean;
  children: ReactNode;
  installation: ReactNode;
  installed: ReactNode;
  nexus?: ReactNode;
}

export function GamePanelCarousel({ mode, reducedMotion, children, installation, installed, nexus }: Props) {
  const frame = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState<number>();
  // Preserve panel dimensions while their contents slide between modes.
  useLayoutEffect(() => {
    const viewport = frame.current;
    if (!viewport) return;
    const measure = () => {
      const panel = viewport.querySelector<HTMLElement>('[data-panel-mode="mods"]');
      if (panel) setHeight(panel.offsetHeight);
    };
    const observer = new ResizeObserver(measure);
    observer.observe(viewport);
    measure();
    return () => observer.disconnect();
  }, [mode]);

  return <div ref={frame} className="game-panel-carousel" style={{ minHeight: height }}>
    <AnimatePresence mode="wait" initial={false}>
      <motion.div key={mode} data-panel-mode={mode} className="game-detail-panels"
        initial={{ y: reducedMotion ? 0 : -180, opacity: reducedMotion ? 1 : 0 }}
        animate={{ y: 0, opacity: 1 }} exit={{ y: reducedMotion ? 0 : 180, opacity: reducedMotion ? 1 : 0 }}
        transition={{ duration: reducedMotion ? 0 : 0.35, ease: [0.4, 0, 0.2, 1] }}
        style={{ minHeight: height }}>
        {mode === "nexus" ? nexus : mode === "mods" ? children : mode === "installed" ? installed : installation}
      </motion.div>
    </AnimatePresence>
  </div>;
}

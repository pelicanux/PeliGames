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

  // Keep opacity off the glass ancestor so backdrop blur stays stable during slides.
  return <div ref={frame} className="game-panel-carousel" style={{ minHeight: height }}>
    <AnimatePresence mode="wait" initial={false}>
      <motion.div key={mode} data-panel-mode={mode} className="game-detail-panels"
        // Travel the full panel height before AnimatePresence admits the next mode.
        // A fixed pixel offset removes tall panels while they are still visible.
        initial={{ y: reducedMotion ? 0 : "-100%" }}
        animate={{ y: 0 }} exit={{ y: reducedMotion ? 0 : "100%" }}
        transition={{ duration: reducedMotion ? 0 : 0.35, ease: [0.4, 0, 0.2, 1] }}
        style={{ minHeight: height }}>
        {mode === "nexus" ? nexus : mode === "mods" ? children : mode === "installed" ? installed : installation}
      </motion.div>
    </AnimatePresence>
  </div>;
}

import { useEffect } from "react";

const surfaces = ".menu-glass-panel, .modal-content, .dialog-popover, .cover-context-menu, .ui-hover-tooltip, .installation-name-hint, .effects-menu, .custom-select-menu, .text-context-menu, .model-source-tooltip, .launcher-update-tooltip";
const objects = "h2, h3, h4, label, button, input, select, textarea, .installation-field-label, .game-info-label, .info-value-pill, .game-directory-value, .credits-neon-card, .credits-neon-logo, .credits-neon-copy";
// Path fields already have a framed wrapper; their editable input stays unframed.
const controls = "input:not(.installation-path-input):not([type=checkbox]):not([type=radio]):not([type=range]), select, textarea, .custom-select-trigger, .game-directory-value, .info-value-pill, .credits-neon-card, .credits-neon-logo";

/** Measure only after content, size or scroll changes; never run a continuous animation loop. */
export function useSurfaceNeonLighting(appearance: string) {
  useEffect(() => {
    if (appearance !== "dark") return;
    let pending = 0;
    const tracked = new Set<Element>();
    const decorated = new Set<HTMLElement>();
    const illuminate = () => {
      pending = 0;
      const panels = Array.from(document.querySelectorAll<HTMLElement>(surfaces));
      const current = new Set<Element>(panels);
      for (const panel of panels) {
        if (!tracked.has(panel)) { tracked.add(panel); resize.observe(panel); }
      }
      for (const panel of tracked) {
        if (!current.has(panel)) { resize.unobserve(panel); tracked.delete(panel); }
      }
      // Finish layout reads before writing styles to avoid repeated layout work.
      const measurements = panels.flatMap(panel => {
        if (panel.closest(".performance-mode") || !panel.getClientRects().length) return [];
        const bounds = panel.getBoundingClientRect();
        return Array.from(panel.querySelectorAll<HTMLElement>(objects))
          .filter(element => element.closest(surfaces) === panel)
          .map(element => ({ element, bounds, box: element.getBoundingClientRect() }));
      });
      for (const { element, bounds, box } of measurements) {
        const x = box.left + box.width / 2 - bounds.left;
        const y = box.top + box.height / 2 - bounds.top;
        const fromTop = Math.hypot(x, y) <= Math.hypot(bounds.width - x, bounds.height - y);
        const dx = fromTop ? x : x - bounds.width, dy = fromTop ? y : y - bounds.height;
        const distance = Math.max(1, Math.hypot(dx, dy));
        const intensity = Math.exp(-distance / 380);
        const reach = Math.max(0, Math.min(1, (300 - distance) / 180));
        const strength = reach * reach * (3 - 2 * reach);
        const offset = strength * (2 + intensity * 3);
        element.classList.add("neon-lit-object");
        element.classList.toggle("neon-lit-control", element.matches(controls));
        decorated.add(element);
        element.style.setProperty("--surface-neon-strength", strength.toFixed(3));
        element.style.setProperty("--surface-neon-x", `${(dx / distance * offset).toFixed(2)}px`);
        element.style.setProperty("--surface-neon-y", `${(dy / distance * offset).toFixed(2)}px`);
        element.style.setProperty("--surface-neon-edge-x", `${dx >= 0 ? 1 : -1}px`);
        element.style.setProperty("--surface-neon-edge-y", `${dy >= 0 ? 1 : -1}px`);
        element.style.setProperty("--surface-neon-light", (intensity * .20).toFixed(3));
        element.style.setProperty("--surface-neon-edge", (intensity * .36).toFixed(3));
      }
      for (const element of decorated) if (!element.isConnected) decorated.delete(element);
    };
    const schedule = () => { if (!pending) pending = requestAnimationFrame(illuminate); };
    const resize = new ResizeObserver(schedule);
    const changes = new MutationObserver(records => {
      // Log text streaming does not require remeasuring window decorations.
      if (records.some(record => [...record.addedNodes, ...record.removedNodes].some(node => node instanceof Element))) schedule();
    });
    changes.observe(document.body, { childList: true, subtree: true });
    document.addEventListener("scroll", schedule, { capture: true, passive: true });
    schedule();
    return () => {
      resize.disconnect(); changes.disconnect(); cancelAnimationFrame(pending);
      document.removeEventListener("scroll", schedule, true);
      for (const element of decorated) {
        element.classList.remove("neon-lit-object", "neon-lit-control");
        for (const name of Array.from(element.style)) {
          if (name.startsWith("--surface-neon-")) element.style.removeProperty(name);
        }
      }
    };
  }, [appearance]);
}

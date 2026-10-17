import { useLayoutEffect, type RefObject } from "react";

/** Keep visible covers on stable compositor surfaces through hover/dialogs.
 * Offscreen covers release theirs, so a large library does not retain a GPU
 * texture for every game. Observing never changes visibility or mounts cards. */
export function useVisibleCoverLayers(root: RefObject<HTMLElement | null>, enabled: boolean) {
  useLayoutEffect(() => {
    const element = root.current;
    if (!element || !enabled) return;
    const cards = new Set<HTMLElement>();
    const observer = new IntersectionObserver(entries => {
      for (const entry of entries) {
        if (entry.isIntersecting) entry.target.setAttribute("data-cover-composited", "");
        else entry.target.removeAttribute("data-cover-composited");
      }
    }, { rootMargin: "240px 0px" });
    const visit = (node: Node, add: boolean) => {
      if (!(node instanceof HTMLElement)) return;
      const found = [...(node.matches(".library-card-cell") ? [node] : []), ...node.querySelectorAll<HTMLElement>(".library-card-cell")];
      for (const card of found) {
        if (add && !cards.has(card)) { cards.add(card); observer.observe(card); }
        else if (!add && cards.delete(card)) { observer.unobserve(card); card.removeAttribute("data-cover-composited"); }
      }
    };
    visit(element, true);
    const changes = new MutationObserver(records => {
      for (const record of records) {
        record.removedNodes.forEach(node => visit(node, false));
        record.addedNodes.forEach(node => visit(node, true));
      }
    });
    changes.observe(element, { childList: true, subtree: true });
    return () => {
      changes.disconnect(); observer.disconnect();
      cards.forEach(card => card.removeAttribute("data-cover-composited"));
    };
  }, [root, enabled]);
}

import { useLayoutEffect, useRef, type RefObject } from "react";
import { flushSync } from "react-dom";

/** Switch immediately, then settle the content into place without dimming the
 * library, blocking input or moving the surrounding launcher chrome. */
export function useLibraryTransition(root: RefObject<HTMLElement | null>, disabled: boolean) {
  const animation = useRef<Animation | null>(null);
  const reset = () => {
    animation.current?.cancel();
    animation.current = null;
  };
  useLayoutEffect(() => reset, [root, disabled]);
  return (change: () => void, enabled = true) => {
    reset();
    if (!enabled || disabled || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      change();
      return;
    }
    // Commit before animating so there is no outgoing fade or delayed navigation.
    flushSync(change);
    const content = root.current?.querySelector<HTMLElement>(".modern-library");
    if (!content?.animate) return;
    const incoming = content.animate(
      [{ transform: "translateY(6px)" }, { transform: "translateY(0)" }],
      { duration: 180, easing: "cubic-bezier(.16,1,.3,1)" },
    );
    animation.current = incoming;
    void incoming.finished.then(() => {
      if (animation.current === incoming) animation.current = null;
    }).catch(() => {});
  };
}

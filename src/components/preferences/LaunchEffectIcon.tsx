import type { LaunchEffect } from "../../theme/ThemeProvider";

export function LaunchEffectIcon({ effect }: { effect: LaunchEffect }) {
  if (effect === "penguin") return <img src="/icons/tux-color.svg" alt="" />;
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    {effect === "shatter" && <><rect x="5" y="2" width="14" height="20" rx="2" /><path d="m13 2-3 7 5 3-5 4 2 6M5 10l5-1m5 3 4-3M5 18l5-2" /></>}
    {effect === "smooth" && <><rect x="3" y="9" width="7" height="12" rx="1.5" /><path d="m13 11 7-7m-5 0h5v5" /></>}
    {effect === "aquarium" && <><path d="M12 2S5 10 5 14a7 7 0 0 0 14 0c0-4-7-12-7-12Z" /><path d="M8 14q2-2 4 0t4 0M9 18q3 2 6 0" /></>}
    {effect === "particles" && <><rect x="3" y="10" width="10" height="12" rx="1.5" /><path d="M3 15h10M8 10v12" /><circle cx="15" cy="7" r="1" /><circle cx="20" cy="3" r="1" /><circle cx="20" cy="12" r="1" /><circle cx="17" cy="18" r="1" /></>}
    {effect === "shrink" && <><path d="M3 8V3h5m8 0h5v5M3 16v5h5m8 0h5v-5M4 4l5 5m11-5-5 5M4 20l5-5m11 5-5-5" /><rect x="9" y="9" width="6" height="6" rx="1" /></>}
  </svg>;
}

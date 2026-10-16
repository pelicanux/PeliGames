import { CoverHoldProgress } from "./CoverHoldProgress";
import { CoverTuxPeek } from "./CoverTuxPeek";

/** Penguin effect while holding a cover: Tux's head rises from the corner until
 * it is fully shown when the hold completes; the enlarged cover keeps it. */
export function CoverTuxRise({ reducedMotion }: { reducedMotion: boolean }) {
  if (reducedMotion) return <CoverHoldProgress />;
  return <div className="cover-tux-frame" aria-hidden="true"><CoverTuxPeek mode="rise" /></div>;
}

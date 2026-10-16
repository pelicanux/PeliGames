import { useState } from "react";
import { useGameBannerState } from "../hooks/useGameBanner";
import type { GameInfo } from "./GameGrid";
import { SmoothImage } from "./SmoothImage";

export function GameBannerImage({ game, cover }: { game: GameInfo; cover?: string }) {
  const banner = useGameBannerState(game);
  const [failed, setFailed] = useState<string[]>([]);
  // While the banner is still being looked up, show nothing rather than the
  // portrait cover that the banner would replace a moment later.
  const art = banner.url && !failed.includes(banner.url) ? banner.url : banner.done && cover && !failed.includes(cover) ? cover : undefined;
  return art ? <SmoothImage key={art} className={`modern-continue-art${art === cover ? " modern-continue-art--cover" : ""}`} src={art} alt="" aria-hidden="true" onError={() => setFailed(current => current.includes(art) ? current : [...current, art])} /> : null;
}

import { useEffect, useState, type MouseEventHandler, type Ref } from "react";
import { motion } from "framer-motion";
import type { GameInfo } from "../GameGrid";
import { GamePlatformBadge } from "../GamePlatformBadge";

interface Props {
  game: GameInfo;
  preparing: boolean;
  platform?: "native" | "proton";
  coverRef: Ref<HTMLDivElement>;
  onMove: MouseEventHandler<HTMLDivElement>;
  onLeave: MouseEventHandler<HTMLDivElement>;
  onContextMenu: (position: { x: number; y: number }) => void;
}

export function SelectedGameCover({ game, preparing, platform, coverRef, onMove, onLeave, onContextMenu }: Props) {
  const [coverError, setCoverError] = useState(false);
  useEffect(() => {
    setCoverError(false);
    const changed = () => setCoverError(false);
    window.addEventListener("gameCoverChanged", changed);
    return () => window.removeEventListener("gameCoverChanged", changed);
  }, [game.path]);
  return (
    <motion.div
      className={`selected-cover ${preparing ? "is-preparing-game" : ""}`}
      ref={coverRef}
      layout={false}
      onMouseMove={onMove}
      onMouseLeave={onLeave}
      style={{
        position: "relative",
        borderRadius: "12px",
        overflow: "hidden",
        transition: "box-shadow 0.1s ease-out",
        boxShadow: "0 0 20px rgba(var(--accent-rgb), 0.6)",
        border: "2px solid rgba(var(--accent-rgb), 0.8)",
        transformStyle: "flat",
        willChange: "auto",
        cursor: "pointer",
        height: "max-content",
        display: "inline-block"
      }}
      onContextMenu={(e) => { e.preventDefault(); onContextMenu({ x: e.clientX, y: e.clientY }); }}
    >
      <GamePlatformBadge game={game} platform={platform} />
      <div className="glare" style={{
        position: "absolute", top: 0, left: 0, right: 0, bottom: 0,
        pointerEvents: "none", opacity: 0, transition: "opacity 0.2s ease-out", zIndex: 10
      }} />
      {game.cover_url && !coverError ? (
        <img
          src={game.cover_url}
          alt={game.name}
          decoding="async"
          onError={() => setCoverError(true)}
          style={{
            width: "11.25rem",
            objectFit: "cover",
            aspectRatio: "2/3",
            display: "block"
          }}
        />
      ) : (
        <div style={{
            width: "11.25rem", aspectRatio: "2/3",
            background: "linear-gradient(135deg, #1e3a8a, #312e81)",
            position: "relative",
            textAlign: "center"
          }}>
          <span style={{ position: "absolute", top: "1.5rem", left: "1rem", right: "1rem", fontWeight: "bold", fontSize: "1.1rem", textShadow: "0 2px 4px rgba(0,0,0,0.8)" }}>{game.name}</span>
          <img src="/peligames.svg" alt="Icon" style={{ position: "absolute", top: "50%", left: "50%", transform: "translate(-50%, -50%)", width: "3.75rem", height: "3.75rem", opacity: 0.5 }} />
        </div>
      )}
    </motion.div>
  );
}

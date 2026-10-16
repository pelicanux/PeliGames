import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Platform = "native" | "proton";
interface PlatformGame {
  path: string;
  name: string;
  directory?: string;
  executable?: string;
  prefix?: string;
  proton?: string;
  app_id?: string;
  nexus_platform?: Platform;
}

// Share detection between the grid, recent games and selected cover. Limit disk scans.
const detections = new Map<string, Promise<Platform | undefined>>();
let pending = Promise.resolve();
function detectPlatform(game: PlatformGame) {
  const path = game.directory || game.path;
  const key = JSON.stringify([path, game.app_id]);
  let detection = detections.get(key);
  if (!detection) {
    detection = pending.then(async () => {
      try {
        const result = await invoke<{ platform: string }>("analyze_game", {
          path, name: game.name, appId: game.app_id || null,
        });
        if (/windows/i.test(result.platform)) return "proton" as const;
        if (/linux/i.test(result.platform)) return "native" as const;
      } catch { /* An unidentified platform has no badge. */ }
      return undefined;
    });
    detections.set(key, detection);
    pending = detection.then(() => undefined);
  }
  return detection;
}

export function GamePlatformBadge({ platform, game }: { platform?: Platform; game?: PlatformGame }) {
  const known = platform || game?.nexus_platform ||
    (game && (game.prefix?.trim() || game.proton?.trim() || /\.(exe|msi|bat)$/i.test(game.executable || "")) ? "proton" : undefined);
  const identity = JSON.stringify([game?.directory || game?.path, game?.app_id]);
  const [detected, setDetected] = useState<{ identity: string; platform?: Platform }>();
  useEffect(() => {
    if (known || !game) return;
    let active = true;
    void detectPlatform(game).then(value => { if (active) setDetected({ identity, platform: value }); });
    return () => { active = false; };
  }, [known, identity]);
  const resolved = known || (detected?.identity === identity ? detected.platform : undefined);
  if (!resolved) return null;
  const label = resolved === "native" ? "Linux" : "Windows";
  return <span className="game-platform-badge" role="img" aria-label={label} title={label}>
    {resolved === "proton" ? <svg viewBox="0 0 24 24" aria-hidden="true" fill="currentColor">
      <path d="M2 4.4 10.5 3.2v8.1H2zm10-1.4L22 1.5v9.8H12zM2 12.7h8.5v8.1L2 19.6zm10 0h10v9.8L12 21z" />
    </svg> : <span className="game-platform-tux" aria-hidden="true" />}
  </span>;
}

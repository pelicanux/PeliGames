import { useRef, useState } from "react";
import { runGameInstaller, type GameInstallationRequest, type GameInstallationResult } from "../services/gameInstallation";
import { fetchGameCoverByName } from "../services/customCovers";
export function useGameInstaller() {
  const lock = useRef(false);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<GameInstallationResult>();
  const [error, setError] = useState("");
  const start = async (request: GameInstallationRequest) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setResult(undefined); setError("");
    try {
      const cover = request.cover_url ?? await fetchGameCoverByName(request.name).catch(() => null);
      const result = await runGameInstaller({ ...request, cover_url: cover ?? undefined });
      setResult(result);
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
      return result;
    }
    catch (error) { setError(error instanceof Error ? error.message : String(error)); }
    finally { lock.current = false; setBusy(false); }
  };
  return { busy, result, error, start, reset: () => { if (!lock.current) { setResult(undefined); setError(""); } } };
}

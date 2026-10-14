import { useEffect, useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { setMatrixPresence, useMatrixChat } from "./matrixChat";
import type { RunningGameSummary } from "../components/RunningGamesIndicator";

const KEY = "peligames.matrix.gameActivity";
function preferences() {
  try { const saved = JSON.parse(localStorage.getItem(KEY) || "{}"); return { sharing: saved.sharing === true, covers: saved.covers !== false }; }
  catch { return { sharing: false, covers: true }; }
}
let state = { ...preferences(), error: "", name: "", cover: undefined as string | undefined };
const listeners = new Set<() => void>();
const set = (patch: Partial<typeof state>) => { state = { ...state, ...patch }; listeners.forEach(listener => listener()); };
export function setMatrixActivityPreferences(patch: Partial<Pick<typeof state, "sharing" | "covers">>) {
  set(patch);
  try { localStorage.setItem(KEY, JSON.stringify({ sharing: state.sharing, covers: state.covers })); }
  catch { /* The controls still work for this session if storage is unavailable. */ }
}
export function useMatrixActivityPreferences() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, () => state);
}
export const gameActivityName = (status: string): string | null => {
  const match = /^Jogando (.{1,180}) · PeliGames$/u.exec(status);
  return match ? match[1] : null;
};
export function sharedRunningGame(games: RunningGameSummary[], sharing: boolean) {
  const game = sharing ? games.find(game => game.state === "running") : undefined;
  return { name: game?.name.slice(0, 180).replace(/[\u0000-\u001f\u007f]/g, " ").trim() || "", cover: game?.cover };
}
/** Only actual running sessions are shared; prefix/runtime preparation is private. */
export function useMatrixGameActivity(games: RunningGameSummary[]) {
  const { userId } = useMatrixChat();
  const { sharing } = useMatrixActivityPreferences();
  const { name, cover } = sharedRunningGame(games, sharing);
  useEffect(() => { set({ name, cover }); }, [name, cover]);
  useEffect(() => {
    if (!userId) { set({ error: "" }); return; }
    let stopped = false, timer: ReturnType<typeof setTimeout>;
    const publish = async () => {
      try {
        await invoke("matrix_chat_game_activity", { gameName: name || null, userId });
        if (!stopped) {
          set({ error: "" });
          setMatrixPresence(userId, name ? `Jogando ${name} · PeliGames` : "");
        }
      } catch (error) {
        if (!stopped) { set({ error: String(error) }); timer = setTimeout(() => void publish(), 30000); }
      }
    };
    timer = setTimeout(() => void publish(), 500);
    return () => { stopped = true; clearTimeout(timer); };
  }, [userId, name]);
}

import { useEffect, useState } from "react";

/** Track confirmed running sessions independently of the selected library/game. */
export function useRunningGameTimes(games: { id: string; state: string }[]) {
  const runningIds = games.filter(game => game.state === "running").map(game => game.id).sort().join("\n");
  const [times, setTimes] = useState<Record<string, number>>({});
  useEffect(() => {
    const ids = runningIds ? runningIds.split("\n") : [];
    setTimes(previous => {
      if (ids.length === Object.keys(previous).length && ids.every(id => previous[id] !== undefined)) return previous;
      const now = Date.now();
      return Object.fromEntries(ids.map(id => [id, previous[id] ?? now]));
    });
  }, [runningIds]);
  return times;
}

export function GameRunTimer({ startedAt }: { startedAt: number }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => { setNow(Date.now()); const timer = window.setInterval(() => setNow(Date.now()), 1000); return () => clearInterval(timer); }, [startedAt]);
  const seconds = Math.max(0, Math.floor((now - startedAt) / 1000));
  const time = seconds >= 3600
    ? `${Math.floor(seconds / 3600)}:${String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`
    : `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
  return <span className="modern-play-time" aria-live="off">{time}</span>;
}

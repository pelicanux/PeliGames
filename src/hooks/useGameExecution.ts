import { useEffect, useRef, useState } from "react";
import { startGame, cancelGame, executionStatus, isExecutionActive, runProgram as runInPrefix, startShortcut as openShortcut, type ExecutionStatus } from "../services/gameExecution";
export function useGameExecution() {
  const [session, setSession] = useState<ExecutionStatus | null>(null);
  const [pendingPath, setPendingPath] = useState("");
  const [error, setError] = useState("");
  const locked = useRef(false);
  const revision = useRef(0);
  const announcedSession = useRef("");
  useEffect(() => {
    if (session && isExecutionActive(session) && announcedSession.current !== session.id) {
      announcedSession.current = session.id;
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
    }
  }, [session?.id, session?.state]);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      const request = revision.current;
      try {
        const current = await executionStatus();
        if (active && request === revision.current && !locked.current) { setSession(current); }
      } catch (error) { if (active && request === revision.current) setError(String(error)); }
      if (active) timer = setTimeout(poll, 700);
    };
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, []);
  const start = async (path: string, executable?: string) => {
    if (locked.current || isExecutionActive(session)) return;
    locked.current = true; ++revision.current; setPendingPath(path); setError("");
    try { setSession(await startGame(path, executable)); }
    catch (error) { setError(String(error)); }
    finally { locked.current = false; setPendingPath(""); }
  };
  // Programs of the prefix may also open while its game runs (they join the
  // running prefix and leave the game's session as it is).
  const launch = async (path: string, request: () => Promise<ExecutionStatus | null>) => {
    if (locked.current) return;
    locked.current = true; ++revision.current; setPendingPath(path); setError("");
    try { const started = await request(); if (started) setSession(started); }
    catch (error) { setError(String(error)); throw error; }
    finally { locked.current = false; setPendingPath(""); }
  };
  const runProgram = (path: string, executable: string) => launch(path, () => runInPrefix(path, executable));
  const startShortcut = (path: string, id: string) => launch(path, () => openShortcut(path, id));
  const cancel = async () => {
    if (locked.current || !session || !isExecutionActive(session)) return;
    locked.current = true; ++revision.current; setError("");
    try { setSession(await cancelGame(session.id)); }
    catch (error) { setError(String(error)); }
    finally { locked.current = false; }
  };
  return { session, pendingPath, error, start, runProgram, startShortcut, cancel, active: isExecutionActive(session) };
}

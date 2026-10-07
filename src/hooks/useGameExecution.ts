import { useEffect, useRef, useState } from "react";
import { startGame, cancelGame, executionStatus, isExecutionActive, type ExecutionStatus } from "../services/gameExecution";
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
  const cancel = async () => {
    if (locked.current || !session || !isExecutionActive(session)) return;
    locked.current = true; ++revision.current; setError("");
    try { setSession(await cancelGame(session.id)); }
    catch (error) { setError(String(error)); }
    finally { locked.current = false; }
  };
  return { session, pendingPath, error, start, cancel, active: isExecutionActive(session) };
}

import { useEffect, useRef, useState } from "react";
import { startSteamGame, startExternalGame, startGame, cancelGame, executionStatuses, isExecutionActive, runProgram as runInPrefix, startShortcut as openShortcut, type ExecutionStatus } from "../services/gameExecution";
export function useGameExecution() {
  const [session, setSession] = useState<ExecutionStatus | null>(null);
  const [sessions,setSessions]=useState<ExecutionStatus[]>([]);
  const [pendingPath, setPendingPath] = useState("");
  const [cancellingPath, setCancellingPath] = useState("");
  const cancelRequested = useRef(false);
  const [error, setError] = useState("");
  const locked = useRef(false);
  const revision = useRef(0);
  const announcedStates=useRef("");
  const lastStatus = useRef("");
  const apply = (next: ExecutionStatus | null) => { lastStatus.current = ""; setSession(next); if(next)setSessions(previous=>[...previous.filter(item=>item.id!==next.id&&item.path!==next.path),next]); };
  useEffect(()=>{
    const snapshot=JSON.stringify(sessions.map(item=>[item.id,item.path,item.state]));
    if(announcedStates.current!==snapshot){announcedStates.current=snapshot;window.dispatchEvent(new Event("peligamesLibraryChanged"));}
  },[sessions]);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    let running = false;
    const poll = async () => {
      clearTimeout(timer);
      const request = revision.current;
      try {
        const states = await executionStatuses();
        const current=[...states].reverse().find(isExecutionActive)??states[states.length-1]??null;
        running = states.some(isExecutionActive);
        // The same status again keeps the launcher as it is (no re-render).
        const snapshot = JSON.stringify(states);
        if (active && request === revision.current && !locked.current && snapshot !== lastStatus.current) { lastStatus.current = snapshot; setSession(current); setSessions(states); }
      } catch (error) { if (active && request === revision.current) setError(String(error)); }
      // Quick while a game runs and the launcher is visible; calmer otherwise.
      if (active) timer = setTimeout(poll, running && !document.hidden ? 700 : 2000);
    };
    const visible = () => { if (!document.hidden && active) void poll(); };
    document.addEventListener("visibilitychange", visible);
    void poll();
    return () => { active = false; clearTimeout(timer); document.removeEventListener("visibilitychange", visible); };
  }, []);
  const start = async (path: string, executable?: string) => {
    if (locked.current || sessions.some(item=>item.path===path&&isExecutionActive(item))) return;
    locked.current = true; cancelRequested.current = false; ++revision.current; setPendingPath(path); setError("");
    try {
      const started = await startGame(path, executable); apply(started);
      if (cancelRequested.current && isExecutionActive(started)) apply(await cancelGame(started.id));
    }
    catch (error) { setError(String(error)); }
    finally { locked.current = false; cancelRequested.current = false; setPendingPath(""); setCancellingPath(""); }
  };
  // Programs of the prefix may also open while its game runs (they join the
  // running prefix and leave the game's session as it is).
  const launch = async (path: string, request: () => Promise<ExecutionStatus | null>) => {
    if (locked.current) return;
    locked.current = true; cancelRequested.current = false; ++revision.current; setPendingPath(path); setError("");
    try {
      const started = await request(); if (started) apply(started);
      const target = started ?? (session?.path === path ? session : null);
      if (cancelRequested.current && target && isExecutionActive(target)) apply(await cancelGame(target.id));
    }
    catch (error) { setError(String(error)); throw error; }
    finally { locked.current = false; cancelRequested.current = false; setPendingPath(""); setCancellingPath(""); }
  };
  const startSteam = (path: string, appId: string, externalId?: string) => launch(path, () => startSteamGame(path, appId, externalId));
  const startExternal = (path: string, externalId: string) => launch(path, () => startExternalGame(path, externalId));
  const runProgram = (path: string, executable: string) => launch(path, () => runInPrefix(path, executable));
  const startShortcut = (path: string, id: string) => launch(path, () => openShortcut(path, id));
  const cancel = async (path?:string) => {
    const target=path?sessions.find(item=>item.path===path&&isExecutionActive(item)):session;
    if (locked.current && pendingPath && (!path||path===pendingPath)) { cancelRequested.current = true; setCancellingPath(pendingPath); return; }
    if (locked.current || !target || !isExecutionActive(target)) return;
    locked.current = true; ++revision.current; setError(""); setCancellingPath(target.path);
    try { apply(await cancelGame(target.id)); }
    catch (error) { setError(String(error)); }
    finally { locked.current = false; setCancellingPath(""); }
  };
  return { session, sessions, statusFor:(path:string)=>sessions.find(item=>item.path===path&&isExecutionActive(item))??sessions.find(item=>item.path===path), pendingPath, cancellingPath, error, start, startSteam, startExternal, runProgram, startShortcut, cancel, active: sessions.some(isExecutionActive) };
}

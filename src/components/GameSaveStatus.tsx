import {createContext, useCallback, useContext, useEffect, useId, useState, type ReactNode} from "react";
import {useI18n} from "../i18n/I18nContext";

type SaveStatus = "" | "pending" | "saving" | "saved" | "error";
type Notice = {owner: string; status: SaveStatus; changed: number};
const Context = createContext<{notices: Record<string, Notice>; report: (scope: string, owner: string, status: SaveStatus) => void} | null>(null);

export function GameSaveStatusProvider({children}: {children: ReactNode}) {
  const [notices, setNotices] = useState<Record<string, Notice>>({});
  const report = useCallback((scope: string, owner: string, status: SaveStatus) => {
    setNotices(current => {
      const previous = current[scope];
      if (!status && previous?.owner !== owner) return current;
      if (previous?.owner === owner && previous.status === status) return current;
      if (!status) {const next = {...current}; delete next[scope]; return next;}
      return {...current, [scope]: {owner, status, changed: Date.now()}};
    });
  }, []);
  useEffect(() => {
    const timers = Object.entries(notices).filter(([, notice]) => notice.status === "saved")
      .map(([scope, notice]) => window.setTimeout(() => report(scope, notice.owner, ""), Math.max(0, 3000 - (Date.now() - notice.changed))));
    return () => timers.forEach(window.clearTimeout);
  }, [notices, report]);
  return <Context.Provider value={{notices, report}}>{children}</Context.Provider>;
}

/** Each panel owns its feedback; another panel cannot clear or display it. */
export function useGameSaveStatus(status: SaveStatus, scope = "panel") {
  const context = useContext(Context), owner = useId(), report = context?.report;
  useEffect(() => {report?.(scope, owner, status);}, [scope, owner, status, report]);
  useEffect(() => () => report?.(scope, owner, ""), [scope, owner, report]);
  return Boolean(context);
}

export function GameSaveStatusIndicator({scope = "panel"}: {scope?: string}) {
  const {t: tx} = useI18n();

  const context = useContext(Context);
  const status = context?.notices[scope]?.status;
  const text = status === "saving" || status === "pending" ? (tx("ui-game-save-status", "saving"))
    : status === "saved" ? (tx("ui-game-save-status", "saved"))
    : status === "error" ? (tx("ui-game-save-status", "save_failed")) : "";
  if (!context) return null;
  return <span className="modern-game-save-status" data-state={status || "idle"} role="status" aria-live="polite" aria-atomic="true">
    <span className="modern-game-save-badge">
      <svg className="modern-game-save-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
        {status === "saved" ? <><circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/></> : status === "error" ? <><circle cx="12" cy="12" r="9"/><path d="M12 7v6m0 3v1"/></> : <path d="M21 12a9 9 0 1 1-9-9"/>}
      </svg>{text}
    </span>
  </span>;
}

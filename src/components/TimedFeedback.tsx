import { useEffect, useState, type ReactNode } from "react";

/** Each new message is visible for three seconds, without changing operation state. */
export function TimedFeedback({ children, resetKey, className = "installation-cover-feedback", role = "status", as: Tag = "p" }: {
  children: ReactNode; resetKey?: string | number | boolean; className?: string;
  role?: "status" | "alert"; as?: "p" | "div";
}) {
  const message = resetKey ?? children;
  const [visibleMessage, setVisibleMessage] = useState(message);
  useEffect(() => {
    setVisibleMessage(message);
    const timeout = window.setTimeout(() => setVisibleMessage(undefined), 3000);
    return () => window.clearTimeout(timeout);
  }, [message]);
  return visibleMessage === message ? <Tag className={className} role={role} aria-live="polite">{children}</Tag> : null;
}

import type { MatrixMemberPresence, MatrixPresenceChoice } from "../services/matrixPresencePreferences";

/** Missing/offline presence is intentionally unmarked: never infer online from membership. */
export function MatrixPresenceBadge({ status, pt }: { status?: MatrixMemberPresence | MatrixPresenceChoice; pt: boolean }) {
  if (!status || status === "offline" || status === "invisible") return null;
  const label = status === "online" ? "Online" : status === "idle" ? (pt ? "Ausente" : "Idle") : (pt ? "Não perturbe" : "Do not disturb");
  return <i className={`matrix-avatar-presence is-${status}`} role="img" aria-label={label} title={label}/>;
}

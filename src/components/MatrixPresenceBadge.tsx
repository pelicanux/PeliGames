import {useI18n} from "../i18n/I18nContext";
import type { MatrixMemberPresence, MatrixPresenceChoice } from "../services/matrixPresencePreferences";

/** Missing/offline presence is intentionally unmarked: never infer online from membership. */
export function MatrixPresenceBadge({ status }: { status?: MatrixMemberPresence | MatrixPresenceChoice; }) {
  const {t: tx} = useI18n();

  if (!status || status === "offline" || status === "invisible") return null;
  const label = status === "online" ? "Online" : status === "idle" ? (tx("ui-matrix-presence-badge", "idle")) : (tx("ui-matrix-presence-badge", "do_not_disturb"));
  return <i className={`matrix-avatar-presence is-${status}`} role="img" aria-label={label} title={label}/>;
}

import {useI18n} from "../i18n/I18nContext";
// Right-click menu of a person in a call, as in Discord: profile, the volume
// this listener hears them at, mute and block. The profile card shows the
// picture (larger on click), name, presence and the game being played.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { MatrixChannelMenu, copyText } from "./MatrixChannelMenu";
import { MatrixPresenceBadge } from "./MatrixPresenceBadge";
import { displayName, setMatrixUserBlocked, useMatrixChat } from "../services/matrixChat";
import { matrixAvatar } from "../services/matrixMedia";
import { gameActivityName, useMatrixActivityPreferences } from "../services/matrixGameActivity";
import { visibleOwnGame } from "../services/matrixMemberActivity";
import { useMatrixPresenceChoice } from "../services/matrixPresencePreferences";
import { setMatrixUserVolume, useMatrixUserVolume } from "../services/matrixUserVolumes";
import { useNativeVideoOverlay } from "../services/matrixScreenView";
import { notifyError } from "../services/notifications";

export type UserMenuTarget = { user: string; x: number; y: number };

export function MatrixUserMenu({ target, onClose, profile: profileOnly = false }: {
  target: UserMenuTarget; onClose: () => void;
  /** Opens straight on the profile card (from a message's menu). */
  profile?: boolean;
}) {
  const {t: tx} = useI18n();

  const onError = (error: unknown, fallback: string) => notifyError("Matrix", String(error ?? "").trim() || fallback);
  const chat = useMatrixChat();
  const { user, x, y } = target;
  const own = user === chat.userId;
  const volume = useMatrixUserVolume(user);
  const [profile, setProfile] = useState(profileOnly);
  if (profile) return <MatrixProfileCard user={user} x={x} y={y} onClose={onClose}/>;
  const change = (patch: Parameters<typeof setMatrixUserVolume>[1]) =>
    void setMatrixUserVolume(user, patch).catch(error => onError(error, tx("ui-matrix-user-menu", "could_not_change_the_volume")));
  return <MatrixChannelMenu x={x} y={y} title={displayName(user, chat.names)} onClose={onClose} groups={[
    [{ label: tx("ui-matrix-user-menu", "profile"), action: () => setProfile(true) }],
    [{ label: tx("ui-matrix-user-menu", "user_volume"), hidden: own, slider: { value: volume.volume, max: 200, onChange: value => change({ volume: value }) } }],
    [
      { label: tx("ui-matrix-user-menu", "mute"), hidden: own, checked: volume.muted, action: () => change({ muted: !volume.muted }) },
    ],
    [
      { label: tx("ui-matrix-user-menu", "copy_user_id"), action: () => void copyText(user).catch(error => onError(error, tx("ui-matrix-user-menu", "could_not_copy"))) },
      { label: tx("ui-matrix-user-menu", "block"), danger: true, hidden: own || chat.ignored.includes(user),
        action: () => void setMatrixUserBlocked(user, true).catch(error => onError(error, tx("ui-matrix-user-menu", "could_not_block"))) },
    ],
  ]}/>;
}

function MatrixProfileCard({ user, x, y, onClose }: { user: string; x: number; y: number; onClose: () => void }) {
  const {t: tx} = useI18n();

  const chat = useMatrixChat();
  const activity = useMatrixActivityPreferences();
  const choice = useMatrixPresenceChoice(chat.userId);
  const own = user === chat.userId;
  const card = useRef<HTMLDivElement>(null);
  useNativeVideoOverlay(card);
  const [avatar, setAvatar] = useState<string | null>(null), [large, setLarge] = useState(false);
  const [place, setPlace] = useState({ left: x, top: y });
  useEffect(() => { let active = true; void matrixAvatar(user).then(url => { if (active) setAvatar(url); }).catch(() => {}); return () => { active = false; }; }, [user]);
  useLayoutEffect(() => {
    const box = card.current?.getBoundingClientRect();
    if (!box) return;
    setPlace({ left: Math.max(4, Math.min(x, window.innerWidth - box.width - 4)), top: Math.max(4, Math.min(y, window.innerHeight - box.height - 4)) });
    card.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, [x, y, large]);
  const name = displayName(user, chat.names);
  const presence = own ? (choice === "invisible" ? "invisible" : chat.memberPresence[user] ?? choice) : chat.memberPresence[user];
  const presenceLabel = !presence || presence === "offline" || presence === "invisible" ? ("Offline")
    : presence === "online" ? "Online" : presence === "idle" ? (tx("ui-matrix-user-menu", "idle")) : (tx("ui-matrix-user-menu", "do_not_disturb"));
  const game = own ? visibleOwnGame(activity.sharing, choice === "invisible", activity.name) : gameActivityName(chat.presence[user] ?? "");
  return <div className="matrix-channel-menu-layer" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }} onContextMenu={event => { event.preventDefault(); onClose(); }}>
    <div ref={card} className={`matrix-profile-card${large ? " is-large" : ""}`} role="dialog" aria-label={tx("ui-matrix-user-menu", "s_profile", {"name": name})} style={place}
      onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); onClose(); } }}>
      <button type="button" className="matrix-profile-avatar" aria-pressed={large} title={large ? (tx("ui-matrix-user-menu", "shrink_picture")) : (tx("ui-matrix-user-menu", "enlarge_picture"))} onClick={() => setLarge(value => !value)}>
        {avatar ? <img src={avatar} alt=""/> : <span aria-hidden="true">{name.slice(0, 2).toUpperCase()}</span>}
        <MatrixPresenceBadge status={presence}/>
      </button>
      <strong title={name}>{name}{own ? (tx("ui-matrix-user-menu", "you")) : ""}</strong>
      <code title={user}>{user}</code>
      <dl>
        <dt>{"> status"}</dt><dd>{presenceLabel}</dd>
        <dt>{tx("ui-matrix-user-menu", "activity")}</dt><dd>{game ? `${tx("ui-matrix-user-menu", "playing")} · ${game}` : (tx("ui-matrix-user-menu", "not_playing_right_now"))}</dd>
      </dl>
    </div>
  </div>;
}

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

export function MatrixUserMenu({ pt, target, onClose, profile: profileOnly = false }: {
  pt: boolean; target: UserMenuTarget; onClose: () => void;
  /** Opens straight on the profile card (from a message's menu). */
  profile?: boolean;
}) {
  const onError = (error: unknown, fallback: string) => notifyError("Matrix", String(error ?? "").trim() || fallback);
  const chat = useMatrixChat();
  const { user, x, y } = target;
  const own = user === chat.userId;
  const volume = useMatrixUserVolume(user);
  const [profile, setProfile] = useState(profileOnly);
  if (profile) return <MatrixProfileCard pt={pt} user={user} x={x} y={y} onClose={onClose}/>;
  const change = (patch: Parameters<typeof setMatrixUserVolume>[1]) =>
    void setMatrixUserVolume(user, patch).catch(error => onError(error, pt ? "Não foi possível mudar o volume." : "Could not change the volume."));
  return <MatrixChannelMenu x={x} y={y} title={displayName(user, chat.names)} onClose={onClose} groups={[
    [{ label: pt ? "Perfil" : "Profile", action: () => setProfile(true) }],
    [{ label: pt ? "Volume do usuário" : "User volume", hidden: own, slider: { value: volume.volume, max: 200, onChange: value => change({ volume: value }) } }],
    [
      { label: pt ? "Silenciar" : "Mute", hidden: own, checked: volume.muted, action: () => change({ muted: !volume.muted }) },
    ],
    [
      { label: pt ? "Copiar ID do usuário" : "Copy user ID", action: () => void copyText(user).catch(error => onError(error, pt ? "Não foi possível copiar." : "Could not copy.")) },
      { label: pt ? "Bloquear" : "Block", danger: true, hidden: own || chat.ignored.includes(user),
        action: () => void setMatrixUserBlocked(user, true).catch(error => onError(error, pt ? "Não foi possível bloquear." : "Could not block.")) },
    ],
  ]}/>;
}

function MatrixProfileCard({ pt, user, x, y, onClose }: { pt: boolean; user: string; x: number; y: number; onClose: () => void }) {
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
  const presenceLabel = !presence || presence === "offline" || presence === "invisible" ? (pt ? "Offline" : "Offline")
    : presence === "online" ? "Online" : presence === "idle" ? (pt ? "Ausente" : "Idle") : (pt ? "Não perturbe" : "Do not disturb");
  const game = own ? visibleOwnGame(activity.sharing, choice === "invisible", activity.name) : gameActivityName(chat.presence[user] ?? "");
  return <div className="matrix-channel-menu-layer" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }} onContextMenu={event => { event.preventDefault(); onClose(); }}>
    <div ref={card} className={`matrix-profile-card${large ? " is-large" : ""}`} role="dialog" aria-label={pt ? `Perfil de ${name}` : `${name}'s profile`} style={place}
      onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); onClose(); } }}>
      <button type="button" className="matrix-profile-avatar" aria-pressed={large} title={large ? (pt ? "Reduzir foto" : "Shrink picture") : (pt ? "Ampliar foto" : "Enlarge picture")} onClick={() => setLarge(value => !value)}>
        {avatar ? <img src={avatar} alt=""/> : <span aria-hidden="true">{name.slice(0, 2).toUpperCase()}</span>}
        <MatrixPresenceBadge status={presence} pt={pt}/>
      </button>
      <strong title={name}>{name}{own ? (pt ? " (você)" : " (you)") : ""}</strong>
      <code title={user}>{user}</code>
      <dl>
        <dt>{pt ? "> status" : "> status"}</dt><dd>{presenceLabel}</dd>
        <dt>{pt ? "> atividade" : "> activity"}</dt><dd>{game ? `${pt ? "Jogando" : "Playing"} · ${game}` : (pt ? "Nenhum jogo no momento" : "Not playing right now")}</dd>
      </dl>
    </div>
  </div>;
}

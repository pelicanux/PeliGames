// Account identity lives beside the members; audio controls stay below channels.
import { useEffect, useState } from "react";
import { SelectionMenu } from "./SelectionMenu";
import { displayName } from "../services/matrixChat";
import { setMatrixVoiceControls, useMatrixVoice, type NoiseMode } from "../services/matrixVoice";
import { matrixAvatar } from "../services/matrixMedia";
import { setMatrixPresenceChoice, useMatrixPresenceChoice, type MatrixPresenceChoice } from "../services/matrixPresencePreferences";
import { useMatrixActivityPreferences } from "../services/matrixGameActivity";
import { MatrixPresenceBadge } from "./MatrixPresenceBadge";

const icon = (paths: string[]) => <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths.map(d => <path key={d} d={d}/>)}</svg>;
export const MicIcon = ({ off }: { off?: boolean }) => icon(["M12 3a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3Z", "M19 11a7 7 0 0 1-14 0M12 18v3", ...(off ? ["M4 4l16 16"] : [])]);
export const HeadphonesIcon = ({ off }: { off?: boolean }) => icon(["M4 15v-3a8 8 0 0 1 16 0v3", "M4 15h3v5H5a1 1 0 0 1-1-1v-4ZM20 15h-3v5h2a1 1 0 0 0 1-1v-4Z", ...(off ? ["M4 4l16 16"] : [])]);
export const GearIcon = () => icon(["M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z", "M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z"]);

/** Noise suppression choices; `short` fits the narrow account panel. */
export const noiseOptions = (pt: boolean, short = false) => [
  { value: "off", label: pt ? "Desligada" : "Off" },
  { value: "standard", label: short ? (pt ? "Padrão" : "Standard") : pt ? "Padrão (WebRTC)" : "Standard (WebRTC)" },
  { value: "rnnoise", label: short ? "RNNoise" : pt ? "RNNoise (recomendado)" : "RNNoise (recommended)" },
];

/** The signed-in account's picture, or its initials. */
export function MatrixOwnAvatar({ user, name, version = 0, presence, pt = true }: { user: string; name: string; version?: number; presence?: MatrixPresenceChoice; pt?: boolean }) {
  const [avatar, setAvatar] = useState<string | null>(null);
  useEffect(() => { let active = true; void matrixAvatar(user).then(value => { if (active) setAvatar(value); }).catch(() => {}); return () => { active = false; }; }, [user, version]);
  return <span className="matrix-user-avatar">{avatar ? <img src={avatar} alt=""/> : <span aria-hidden="true">{name.slice(0, 2).toUpperCase()}</span>}<MatrixPresenceBadge status={presence} pt={pt}/></span>;
}

export function MatrixUserProfile({ pt, me, names, avatarVersion, connected }: {
  pt: boolean; me: string; names: Record<string, string>; avatarVersion: number; connected: boolean;
}) {
  const voice = useMatrixVoice();
  const presence = useMatrixPresenceChoice(me);
  const activity = useMatrixActivityPreferences();
  const name = displayName(me, names);
  const speaking = voice.phase === "connected" && voice.speaking.includes(me);
  const options = [
    { value: "online", label: "Online" },
    { value: "idle", label: pt ? "Ausente" : "Idle" },
    { value: "dnd", label: pt ? "Não perturbe" : "Do not disturb" },
    { value: "invisible", label: pt ? "Invisível" : "Invisible" },
  ];
  return <div className={`matrix-own-profile matrix-user-identity${speaking ? " is-speaking" : ""}`} title={me}>
    <MatrixOwnAvatar user={me} name={name} version={avatarVersion} presence={connected ? presence : undefined} pt={pt}/>
    <div className="matrix-own-profile-info"><b>{name}</b>
      {connected ? <SelectionMenu className="matrix-user-presence" menuClassName="matrix-select-menu matrix-presence-menu" minMenuWidth={180} label={pt ? "Alterar status" : "Change status"} value={presence} options={options}
        onChange={value => setMatrixPresenceChoice(me, value as MatrixPresenceChoice)}/>
        : <small>{pt ? "Desconectado" : "Disconnected"}</small>}
      {activity.sharing && activity.name && presence !== "invisible" && <small className="matrix-own-game" title={activity.name}>{pt ? "Jogando" : "Playing"} · {activity.name}</small>}
    </div>
  </div>;
}

export function MatrixUserPanel({ pt, onSettings }: { pt: boolean; onSettings: () => void }) {
  const voice = useMatrixVoice();
  const busy = voice.phase === "joining" || voice.phase === "leaving";
  return <section className="matrix-user-dock" aria-label={pt ? "Controles de áudio" : "Audio controls"}>
    <label className="matrix-user-noise"><span>{pt ? "> ruído" : "> noise"}</span>
      <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={pt ? "Redução de ruído" : "Noise suppression"} value={voice.noise} disabled={busy} onChange={value => void setMatrixVoiceControls({ noise: value as NoiseMode })} options={noiseOptions(pt, true)}/>
    </label>
    <div className="matrix-user-row">
      <span className="matrix-user-buttons">
        <button type="button" className={voice.muted ? "is-off" : ""} aria-pressed={!voice.muted} disabled={busy} title={voice.muted ? (pt ? "Ativar microfone" : "Unmute") : (pt ? "Silenciar microfone" : "Mute")} aria-label={voice.muted ? (pt ? "Ativar microfone" : "Unmute") : (pt ? "Silenciar microfone" : "Mute")}
          onClick={() => void setMatrixVoiceControls(voice.deafened ? { muted: false, deafened: false } : { muted: !voice.muted })}><MicIcon off={voice.muted || voice.deafened}/></button>
        <button type="button" className={voice.deafened ? "is-off" : ""} aria-pressed={voice.deafened} disabled={busy} title={voice.deafened ? (pt ? "Voltar a ouvir" : "Undeafen") : (pt ? "Silenciar áudio" : "Deafen")} aria-label={voice.deafened ? (pt ? "Voltar a ouvir" : "Undeafen") : (pt ? "Silenciar áudio" : "Deafen")}
          onClick={() => void setMatrixVoiceControls({ deafened: !voice.deafened })}><HeadphonesIcon off={voice.deafened}/></button>
        <button type="button" title={pt ? "Configurações de perfil e áudio" : "Profile and audio settings"} aria-label={pt ? "Configurações de perfil e áudio" : "Profile and audio settings"} onClick={onSettings}><GearIcon/></button>
      </span>
    </div>
  </section>;
}

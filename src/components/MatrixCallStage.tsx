import {useI18n} from "../i18n/I18nContext";
// The call view of a voice channel, as in Discord: who is in the call, the
// screen being watched in focus, and the call controls. The video itself is a
// native layer placed over the focus area (see matrixScreenView).
import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { leaveMatrixVoice, setMatrixVoiceControls, useMatrixVoice, watchMatrixScreen } from "../services/matrixVoice";
import { MatrixShareButton } from "./MatrixShareButton";
import { setCallFullscreen, setCallStageShown, useCallFullscreen, useNativeVideo, useMatrixScreenFit, setMatrixScreenFit, type ScreenFit } from "../services/matrixScreenView";
import { matrixAvatar } from "../services/matrixMedia";
import { HeadphonesIcon, MicIcon } from "./MatrixUserPanel";
import { MatrixUserMenu, type UserMenuTarget } from "./MatrixUserMenu";
import { useMatrixUserVolume } from "../services/matrixUserVolumes";

export const ScreenIcon = ({ off }: { off?: boolean }) => <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M8 20h8M12 16v4"/>{off ? <path d="M4 3l16 16"/> : <path d="M12 13V7M9.5 9.5 12 7l2.5 2.5"/>}</svg>;
export const LeaveIcon = () => <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M3.5 14.5c4.7-4.2 12.3-4.2 17 0l-2.2 2.6-3.1-1.6v-2.6a12 12 0 0 0-6.4 0v2.6l-3.1 1.6-2.2-2.6Z"/></svg>;

const PeopleIcon = () => <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><circle cx="9" cy="8" r="3.2"/><path d="M3 19c0-3.3 2.7-5.5 6-5.5s6 2.2 6 5.5"/><circle cx="17" cy="9" r="2.4"/><path d="M16.5 13.6c2.7.2 4.5 2.1 4.5 4.9"/></svg>;
/** Whether the people strip under a watched screen is collapsed; kept between calls. */
const MEMBERS_HIDDEN = "peligames.matrix.callMembersHidden";
/** Marks on a tile a person this listener muted. */
function MutedForMe({ user }: { user: string; }) {
  const {t: tx} = useI18n();

  const { muted } = useMatrixUserVolume(user);
  return muted ? <small>{tx("ui-matrix-call-stage", "muted_for_you")}</small> : null;
}

function Avatar({ user, name }: { user: string; name: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => { let active = true; void matrixAvatar(user).then(url => { if (active) setSrc(url); }).catch(() => {}); return () => { active = false; }; }, [user]);
  return <span className="matrix-call-avatar" aria-hidden="true">{src ? <img src={src} alt=""/> : name.slice(0, 2).toUpperCase()}</span>;
}

export function MatrixCallStage({ me, roomName, videoHidden, onShowChat }: {
  me: string; roomName: string;
  /** Something of the page covers the stage (a dialog): the native video steps aside. */
  videoHidden: boolean;
  /** Back to the text chat of the channel; a watched screen goes to the mini player. */
  onShowChat: () => void;
}) {
  const {t: tx} = useI18n();

  const voice = useMatrixVoice();
  const video = useRef<HTMLDivElement>(null), fullVideo = useRef<HTMLDivElement>(null), stage = useRef<HTMLElement>(null);
  const fullscreen = useCallFullscreen();
  const fit = useMatrixScreenFit();
  const [menu, setMenu] = useState<UserMenuTarget | null>(null);
  const [membersHidden, setMembersHidden] = useState(() => { try { return localStorage.getItem(MEMBERS_HIDDEN) === "1"; } catch { return false; } });
  const toggleMembers = () => {
    const hidden = !membersHidden;
    setMembersHidden(hidden);
    try { localStorage.setItem(MEMBERS_HIDDEN, hidden ? "1" : "0"); } catch { /* This session only. */ }
  };
  const fullControls = useRef<HTMLDivElement>(null);
  const [controlsVisible, setControlsVisible] = useState(true);
  const controlsHovered = useRef(false);
  const controlsTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const wakeControls = useCallback(() => {
    setControlsVisible(true);
    if (controlsTimer.current !== null) clearTimeout(controlsTimer.current);
    controlsTimer.current = setTimeout(() => {
      controlsTimer.current = null;
      const focused = document.activeElement;
      const keyboardFocus = focused?.matches(":focus-visible") && fullControls.current?.contains(focused);
      if (!controlsHovered.current && !keyboardFocus) setControlsVisible(false);
    }, 3000);
  }, []);
  // Full screen covers the Matrix frame only; it ends with the stage (another room, chat closed).
  const [frame, setFrame] = useState<HTMLElement | null>(null);
  useEffect(() => { setFrame(stage.current?.closest<HTMLElement>(".matrix-terminal-frame") ?? null); }, []);
  useEffect(() => () => setCallFullscreen(false), []);
  const connected = voice.phase === "connected";
  const focused = voice.watching;
  useEffect(() => { setCallStageShown(Boolean(focused)); return () => setCallStageShown(false); }, [focused]);
  const full = fullscreen && Boolean(focused) && Boolean(frame);
  useEffect(() => {
    controlsHovered.current = false;
    if (full) wakeControls();
    return () => {
      if (controlsTimer.current !== null) clearTimeout(controlsTimer.current);
      controlsTimer.current = null;
    };
  }, [full, wakeControls]);
  useEffect(() => { if (!focused) setCallFullscreen(false); }, [focused]);
  useNativeVideo(full ? fullVideo : video, Boolean(focused) && !videoHidden, full ? { crt: true, fill: fit === "fill", controls: fullControls } : undefined);
  // Esc leaves full screen first, before it can close the Matrix window.
  useEffect(() => {
    if (!full) return;
    const key = (event: KeyboardEvent) => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); setCallFullscreen(false); } };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [full]);
  const userOf = (identity: string) => identity.split(":").slice(0, 2).join(":");
  // One tile per device in the call, the own one first; "local" is this launcher's share.
  let ownSeen = false;
  const tiles = voice.participants.map(participant => {
    const own = userOf(participant.id) === me && !ownSeen && (ownSeen = true);
    const share = own ? (voice.screen === "live" ? "local" : null) : voice.shares.includes(participant.id) ? participant.id : null;
    return { ...participant, user: userOf(participant.id), own, share };
  }).sort((a, b) => Number(b.own) - Number(a.own));
  const focusedName = focused === "local" ? (tx("ui-matrix-call-stage", "your_screen")) : tiles.find(tile => tile.id === focused)?.name ?? "";
  const busy = voice.phase === "joining" || voice.phase === "leaving";
  return <section ref={stage} className={`matrix-terminal-room matrix-call-stage${focused ? " is-watching" : ""}`} aria-label={tx("ui-matrix-call-stage", "call_in", {"roomname": roomName})}>
    <header><div className="matrix-room-heading"><strong title={roomName}>🔊 {roomName}</strong><small className="matrix-room-topic">{connected ? (tx("ui-matrix-call-stage", "in_the_call", {"tiles_length": tiles.length})) : voice.phase === "reconnecting" ? (tx("ui-matrix-call-stage", "reconnecting")) : (tx("ui-matrix-call-stage", "connecting"))}</small></div>
      <button type="button" className="btn btn-secondary" onClick={onShowChat}>{focused ? (tx("ui-matrix-call-stage", "minimize_and_open_chat")) : (tx("ui-matrix-call-stage", "open_chat"))}</button></header>
    <div className="matrix-call-body">
      {focused && <div className="matrix-call-focus">
        <div ref={video} className="matrix-call-video" onDoubleClick={() => setCallFullscreen(true)}><span>{tx("ui-matrix-call-stage", "loading_stream")}</span></div>
        <div className="matrix-call-focus-bar"><span className="matrix-call-live">{tx("ui-matrix-call-stage", "live")}</span><b>{focusedName}</b>
          <button type="button" onClick={() => setCallFullscreen(true)}>{tx("ui-matrix-call-stage", "full_screen")}</button>
          <button type="button" onClick={onShowChat}>{tx("ui-matrix-call-stage", "minimize")}</button>
          <button type="button" onClick={() => void watchMatrixScreen(null)}>{focused === "local" ? (tx("ui-matrix-call-stage", "close_preview")) : (tx("ui-matrix-call-stage", "stop_watching"))}</button></div>
      </div>}
      {focused && <button type="button" className="matrix-call-members-toggle" aria-expanded={!membersHidden} onClick={toggleMembers}
        title={membersHidden ? (tx("ui-matrix-call-stage", "show_members")) : (tx("ui-matrix-call-stage", "hide_members"))}
        aria-label={membersHidden ? (tx("ui-matrix-call-stage", "show_members")) : (tx("ui-matrix-call-stage", "hide_members"))}>
        <span aria-hidden="true">{membersHidden ? "▴" : "▾"}</span><PeopleIcon/></button>}
      {!(focused && membersHidden) && <ul className={focused ? "matrix-call-strip" : "matrix-call-grid"}>
        {tiles.map(tile => {
          const speaking = connected && voice.speaking.includes(tile.user);
          const watching = tile.share !== null && tile.share === focused;
          return <li key={tile.id} onContextMenu={event => { event.preventDefault(); setMenu({ user: tile.user, x: event.clientX, y: event.clientY }); }} className={[speaking ? "is-speaking" : "", tile.share ? "is-sharing" : "", watching ? "is-watched" : ""].join(" ").trim() || undefined}>
            <Avatar user={tile.user} name={tile.name}/>
            <span className="matrix-call-name">{tile.name}{tile.own ? (tx("ui-matrix-call-stage", "you")) : ""}{tile.own && (voice.muted || voice.deafened) ? <small>{voice.deafened ? (tx("ui-matrix-call-stage", "deafened")) : (tx("ui-matrix-call-stage", "muted"))}</small> : null}{!tile.own && <MutedForMe user={tile.user}/>}</span>
            {tile.share && <span className="matrix-call-live">{tx("ui-matrix-call-stage", "live")}</span>}
            {tile.share && !watching && <button type="button" className="matrix-call-watch" onClick={() => void watchMatrixScreen(tile.share)}>{tile.share === "local" ? (tx("ui-matrix-call-stage", "preview")) : (tx("ui-matrix-call-stage", "watch"))}</button>}
          </li>;
        })}
      </ul>}
    </div>
    <div className="matrix-call-controls" role="toolbar" aria-label={tx("ui-matrix-call-stage", "call_controls")}>
      <button type="button" className={voice.muted || voice.deafened ? "is-off" : ""} aria-pressed={!voice.muted} disabled={busy} title={voice.muted ? (tx("ui-matrix-call-stage", "unmute")) : (tx("ui-matrix-call-stage", "mute"))} aria-label={voice.muted ? (tx("ui-matrix-call-stage", "unmute")) : (tx("ui-matrix-call-stage", "mute"))}
        onClick={() => void setMatrixVoiceControls(voice.deafened ? { muted: false, deafened: false } : { muted: !voice.muted })}><MicIcon off={voice.muted || voice.deafened}/></button>
      <button type="button" className={voice.deafened ? "is-off" : ""} aria-pressed={voice.deafened} disabled={busy} title={voice.deafened ? (tx("ui-matrix-call-stage", "undeafen")) : (tx("ui-matrix-call-stage", "deafen"))} aria-label={voice.deafened ? (tx("ui-matrix-call-stage", "undeafen")) : (tx("ui-matrix-call-stage", "deafen"))}
        onClick={() => void setMatrixVoiceControls({ deafened: !voice.deafened })}><HeadphonesIcon off={voice.deafened}/></button>
      <MatrixShareButton icon={<ScreenIcon off={voice.screen === "live"}/>}/>
      <button type="button" className="matrix-call-leave" disabled={voice.phase === "leaving"} title={tx("ui-matrix-call-stage", "leave_call")} aria-label={tx("ui-matrix-call-stage", "leave_call")} onClick={() => void leaveMatrixVoice()}><LeaveIcon/></button>
    </div>
    {menu && <MatrixUserMenu target={menu} onClose={() => setMenu(null)}/>}
    {full && frame && createPortal(<div className={`matrix-call-fullscreen${controlsVisible ? " is-controls-visible" : ""}`} role="dialog" aria-label={tx("ui-matrix-call-stage", "s_stream_in_full_screen", {"focusedname": focusedName})}
      onPointerEnter={wakeControls} onPointerMove={wakeControls} onPointerDown={wakeControls} onKeyDown={wakeControls}>
      <div ref={fullVideo} className="matrix-call-video" onDoubleClick={() => setCallFullscreen(false)}><span>{tx("ui-matrix-call-stage", "loading_stream")}</span></div>
      <div ref={fullControls} className="matrix-call-focus-bar"
        onPointerEnter={() => { controlsHovered.current = true; wakeControls(); }}
        onPointerLeave={() => { controlsHovered.current = false; wakeControls(); }}
        onFocus={wakeControls} onBlur={wakeControls}><span className="matrix-call-live">{tx("ui-matrix-call-stage", "live")}</span><b>{focusedName}</b>
        <div className="matrix-call-fullscreen-actions">
          <select className="matrix-call-fit" aria-label={tx("ui-matrix-call-stage", "stream_framing")} value={fit} onChange={event => setMatrixScreenFit(event.target.value as ScreenFit)}>
            <option value="fill">{tx("ui-matrix-call-stage", "fill_the_frame")}</option>
            <option value="contain">{tx("ui-matrix-call-stage", "fit_the_entire_picture")}</option>
          </select>
          <button type="button" onClick={() => setCallFullscreen(false)}>{tx("ui-matrix-call-stage", "exit_full_screen_esc")}</button>
          <button type="button" onClick={() => void watchMatrixScreen(null)}>{focused === "local" ? (tx("ui-matrix-call-stage", "close_preview")) : (tx("ui-matrix-call-stage", "stop_watching"))}</button>
        </div></div>
    </div>, frame)}
  </section>;
}

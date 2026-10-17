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

export const ScreenIcon = ({ off }: { off?: boolean }) => <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="1.5"/><path d="M8 20h8M12 16v4"/>{off ? <path d="M4 3l16 16"/> : <path d="M12 13V7M9.5 9.5 12 7l2.5 2.5"/>}</svg>;
const LeaveIcon = () => <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M3.5 14.5c4.7-4.2 12.3-4.2 17 0l-2.2 2.6-3.1-1.6v-2.6a12 12 0 0 0-6.4 0v2.6l-3.1 1.6-2.2-2.6Z"/></svg>;

function Avatar({ user, name }: { user: string; name: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => { let active = true; void matrixAvatar(user).then(url => { if (active) setSrc(url); }).catch(() => {}); return () => { active = false; }; }, [user]);
  return <span className="matrix-call-avatar" aria-hidden="true">{src ? <img src={src} alt=""/> : name.slice(0, 2).toUpperCase()}</span>;
}

export function MatrixCallStage({ pt, me, roomName, videoHidden, onShowChat }: {
  pt: boolean; me: string; roomName: string;
  /** Something of the page covers the stage (a dialog): the native video steps aside. */
  videoHidden: boolean;
  /** Back to the text chat of the channel; a watched screen goes to the mini player. */
  onShowChat: () => void;
}) {
  const voice = useMatrixVoice();
  const video = useRef<HTMLDivElement>(null), fullVideo = useRef<HTMLDivElement>(null), stage = useRef<HTMLElement>(null);
  const fullscreen = useCallFullscreen();
  const fit = useMatrixScreenFit();
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
  const focusedName = focused === "local" ? (pt ? "Sua tela" : "Your screen") : tiles.find(tile => tile.id === focused)?.name ?? "";
  const busy = voice.phase === "joining" || voice.phase === "leaving";
  return <section ref={stage} className={`matrix-terminal-room matrix-call-stage${focused ? " is-watching" : ""}`} aria-label={pt ? `Chamada em ${roomName}` : `Call in ${roomName}`}>
    <header><div className="matrix-room-heading"><strong title={roomName}>🔊 {roomName}</strong><small className="matrix-room-topic">{connected ? (pt ? `${tiles.length} na chamada` : `${tiles.length} in the call`) : voice.phase === "reconnecting" ? (pt ? "reconectando…" : "reconnecting…") : (pt ? "conectando…" : "connecting…")}</small></div>
      <button type="button" className="btn btn-secondary" onClick={onShowChat}>{focused ? (pt ? "Minimizar e abrir o chat" : "Minimize and open chat") : (pt ? "Abrir o chat" : "Open chat")}</button></header>
    <div className="matrix-call-body">
      {focused && <div className="matrix-call-focus">
        <div ref={video} className="matrix-call-video" onDoubleClick={() => setCallFullscreen(true)}><span>{pt ? "> carregando transmissão…" : "> loading stream…"}</span></div>
        <div className="matrix-call-focus-bar"><span className="matrix-call-live">{pt ? "AO VIVO" : "LIVE"}</span><b>{focusedName}</b>
          <button type="button" onClick={() => setCallFullscreen(true)}>{pt ? "[tela cheia]" : "[full screen]"}</button>
          <button type="button" onClick={onShowChat}>{pt ? "[minimizar]" : "[minimize]"}</button>
          <button type="button" onClick={() => void watchMatrixScreen(null)}>{focused === "local" ? (pt ? "[fechar prévia]" : "[close preview]") : (pt ? "[parar de assistir]" : "[stop watching]")}</button></div>
      </div>}
      <ul className={focused ? "matrix-call-strip" : "matrix-call-grid"}>
        {tiles.map(tile => {
          const speaking = connected && voice.speaking.includes(tile.user);
          const watching = tile.share !== null && tile.share === focused;
          return <li key={tile.id} className={[speaking ? "is-speaking" : "", tile.share ? "is-sharing" : "", watching ? "is-watched" : ""].join(" ").trim() || undefined}>
            <Avatar user={tile.user} name={tile.name}/>
            <span className="matrix-call-name">{tile.name}{tile.own ? (pt ? " (você)" : " (you)") : ""}{tile.own && (voice.muted || voice.deafened) ? <small>{voice.deafened ? (pt ? " · sem áudio" : " · deafened") : (pt ? " · mudo" : " · muted")}</small> : null}</span>
            {tile.share && <span className="matrix-call-live">{pt ? "AO VIVO" : "LIVE"}</span>}
            {tile.share && !watching && <button type="button" className="matrix-call-watch" onClick={() => void watchMatrixScreen(tile.share)}>{tile.share === "local" ? (pt ? "[ver prévia]" : "[preview]") : (pt ? "[assistir]" : "[watch]")}</button>}
          </li>;
        })}
      </ul>
    </div>
    <div className="matrix-call-controls" role="toolbar" aria-label={pt ? "Controles da chamada" : "Call controls"}>
      <button type="button" className={voice.muted || voice.deafened ? "is-off" : ""} aria-pressed={!voice.muted} disabled={busy} title={voice.muted ? (pt ? "Ativar microfone" : "Unmute") : (pt ? "Silenciar microfone" : "Mute")} aria-label={voice.muted ? (pt ? "Ativar microfone" : "Unmute") : (pt ? "Silenciar microfone" : "Mute")}
        onClick={() => void setMatrixVoiceControls(voice.deafened ? { muted: false, deafened: false } : { muted: !voice.muted })}><MicIcon off={voice.muted || voice.deafened}/></button>
      <button type="button" className={voice.deafened ? "is-off" : ""} aria-pressed={voice.deafened} disabled={busy} title={voice.deafened ? (pt ? "Voltar a ouvir" : "Undeafen") : (pt ? "Silenciar áudio" : "Deafen")} aria-label={voice.deafened ? (pt ? "Voltar a ouvir" : "Undeafen") : (pt ? "Silenciar áudio" : "Deafen")}
        onClick={() => void setMatrixVoiceControls({ deafened: !voice.deafened })}><HeadphonesIcon off={voice.deafened}/></button>
      <MatrixShareButton pt={pt} icon={<ScreenIcon off={voice.screen === "live"}/>}/>
      <button type="button" className="matrix-call-leave" disabled={voice.phase === "leaving"} title={pt ? "Sair da chamada" : "Leave call"} aria-label={pt ? "Sair da chamada" : "Leave call"} onClick={() => void leaveMatrixVoice()}><LeaveIcon/></button>
    </div>
    {full && frame && createPortal(<div className={`matrix-call-fullscreen${controlsVisible ? " is-controls-visible" : ""}`} role="dialog" aria-label={pt ? `Transmissão de ${focusedName} em tela cheia` : `${focusedName}'s stream in full screen`}
      onPointerEnter={wakeControls} onPointerMove={wakeControls} onPointerDown={wakeControls} onKeyDown={wakeControls}>
      <div ref={fullVideo} className="matrix-call-video" onDoubleClick={() => setCallFullscreen(false)}><span>{pt ? "> carregando transmissão…" : "> loading stream…"}</span></div>
      <div ref={fullControls} className="matrix-call-focus-bar"
        onPointerEnter={() => { controlsHovered.current = true; wakeControls(); }}
        onPointerLeave={() => { controlsHovered.current = false; wakeControls(); }}
        onFocus={wakeControls} onBlur={wakeControls}><span className="matrix-call-live">{pt ? "AO VIVO" : "LIVE"}</span><b>{focusedName}</b>
        <div className="matrix-call-fullscreen-actions">
          <select className="matrix-call-fit" aria-label={pt ? "Enquadramento da transmissão" : "Stream framing"} value={fit} onChange={event => setMatrixScreenFit(event.target.value as ScreenFit)}>
            <option value="fill">{pt ? "Preencher a moldura" : "Fill the frame"}</option>
            <option value="contain">{pt ? "Enquadrar imagem inteira" : "Fit the entire picture"}</option>
          </select>
          <button type="button" onClick={() => setCallFullscreen(false)}>{pt ? "[sair da tela cheia · Esc]" : "[exit full screen · Esc]"}</button>
          <button type="button" onClick={() => void watchMatrixScreen(null)}>{focused === "local" ? (pt ? "[fechar prévia]" : "[close preview]") : (pt ? "[parar de assistir]" : "[stop watching]")}</button>
        </div></div>
    </div>, frame)}
  </section>;
}

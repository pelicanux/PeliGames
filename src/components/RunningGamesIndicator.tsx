import { memo, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";
import { HoverTooltip } from "./HoverTooltip";
import "./runningGamesIndicator.css";

export interface RunningGameSummary {
  id: string; name: string; cover?: string; library: string;
  state: "starting" | "running" | "stopping";
  onDetails?: () => void;
  stopDisabled?: boolean; onStop: () => Promise<unknown>;
}

// Status updates keep identical geometry, so finishing launch preparation does
// not resize the track or change the compositor animation's timing.
const RunningGameMarquee = memo(function RunningGameMarquee({ games, pt }: { games: RunningGameSummary[]; pt: boolean }) {
  const track = useRef<HTMLSpanElement>(null);
  const labels = pt ? ["Preparando:", "Em execução:", "Encerrando:"] : ["Preparing:", "Running:", "Stopping:"];
  useLayoutEffect(() => {
    const element = track.current;
    if (!element) return;
    let distance = 0;
    let duration = 16000;
    const measure = (width: number) => {
      const nextDistance = width / 2 + 16;
      if (!nextDistance || Math.abs(nextDistance - distance) < .5) return;
      const previous = element.getAnimations()[0];
      const elapsed = Number(previous?.currentTime ?? 0);
      const travelled = distance ? (elapsed % duration) / duration * distance : 0;
      distance = nextDistance;
      duration = distance / 28 * 1000;
      element.style.setProperty("--marquee-distance", `${-distance}px`);
      element.style.animationDuration = `${duration}ms`;
      const animation = element.getAnimations()[0];
      if (animation) animation.currentTime = (travelled % distance) / distance * duration;
    };
    measure(element.offsetWidth);
    const observer = new ResizeObserver(entries => {
      const size = entries[0]?.borderBoxSize[0];
      if (size) measure(size.inlineSize);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return <span className="titlebar-running-viewport"><span ref={track} className="titlebar-running-track" aria-hidden="true">
    {[0, 1].map(copy => <span key={copy}>{games.map((game, index) => <span className="titlebar-running-entry" key={game.id}>
      {index > 0 && "   /   "}
      <span className="titlebar-running-status">
        {labels.map(label => <span className="titlebar-running-status-size" key={label}>{label}</span>)}
        <span>{labels[game.state === "starting" ? 0 : game.state === "stopping" ? 2 : 1]}</span>
      </span>{" "}{game.name}
    </span>)}</span>)}
  </span></span>;
}, (before, after) => before.pt === after.pt && before.games.length === after.games.length && before.games.every((game, index) => {
  const next = after.games[index];
  return game.id === next.id && game.name === next.name && game.state === next.state;
}));

export function RunningGamesIndicator({ games }: { games: RunningGameSummary[] }) {
  const { language } = useI18n(), pt = language === "pt";
  const [open, setOpen] = useState(false), [stopping, setStopping] = useState<string | null>(null), [error, setError] = useState("");
  const [position, setPosition] = useState({ left: 0, top: 50 });
  const trigger = useRef<HTMLButtonElement>(null), popup = useRef<HTMLDivElement>(null);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pinned = useRef(false);
  const panelId = useId();
  const label = (game: RunningGameSummary) => game.state === "starting" ? pt ? "Preparando" : "Preparing" : game.state === "stopping" ? pt ? "Encerrando" : "Stopping" : pt ? "Em execução" : "Running";
  const message = games.map(game => `${label(game)}: ${game.name}`).join("   /   ");
  const close = () => { clearTimeout(hideTimer.current); pinned.current = false; setOpen(false); };
  const show = () => { clearTimeout(hideTimer.current); setOpen(true); };
  const scheduleClose = () => { clearTimeout(hideTimer.current); hideTimer.current = setTimeout(() => { if (!pinned.current && !popup.current?.contains(document.activeElement)) setOpen(false); }, 180); };
  useEffect(() => () => clearTimeout(hideTimer.current), []);
  useEffect(() => { if (!games.length) { pinned.current = false; setOpen(false); setError(""); } }, [games.length]);
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      const rect = trigger.current?.getBoundingClientRect();
      if (rect) setPosition({left: Math.max(8, Math.min(rect.left, window.innerWidth - (popup.current?.getBoundingClientRect().width || 224) - 8)), top: rect.bottom + 8});
    };
    place(); window.addEventListener("resize", place);
    const outside = (event: PointerEvent) => { if (event.target instanceof Node && !trigger.current?.contains(event.target) && !popup.current?.contains(event.target)) close(); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); close(); trigger.current?.focus(); } };
    document.addEventListener("pointerdown", outside); window.addEventListener("keydown", escape, true);
    return () => { window.removeEventListener("resize",place); document.removeEventListener("pointerdown",outside); window.removeEventListener("keydown",escape,true); };
  }, [open]);
  if (!games.length) return null;
  const stop = async (game: RunningGameSummary) => {
    if (stopping || game.stopDisabled) return;
    setStopping(game.id); setError("");
    try { if (await game.onStop() === false) setError(pt ? "Não foi possível encerrar o jogo. Confira as notificações." : "Could not stop the game. Check notifications."); }
    catch (failure) { setError(String(failure)); }
    finally { setStopping(null); }
  };
  return <>
    <button ref={trigger} type="button" className="titlebar-running-game" data-tauri-drag-region="false" aria-label={message} aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? panelId : undefined}
      onMouseEnter={show} onMouseLeave={scheduleClose} onClick={() => { if (pinned.current) close(); else { pinned.current = true; show(); } }} onKeyDown={event => { if (event.key === "ArrowDown") { event.preventDefault(); pinned.current = true; show(); requestAnimationFrame(() => popup.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus()); } }}>
      <i aria-hidden="true" />
      <RunningGameMarquee games={games} pt={pt} />
    </button>
    {open && createPortal(<div ref={popup} id={panelId} role="dialog" aria-label={pt ? "Jogos em execução" : "Running games"} className="titlebar-running-popup dialog-popover" style={position} onMouseEnter={show} onMouseLeave={scheduleClose}>
      {games.map(game => <article key={game.id}>
        <HoverTooltip text={pt ? "Detalhes" : "Details"} anchorClassName="titlebar-running-details-anchor">
          <button type="button" className="titlebar-running-details" aria-label={pt ? "Detalhes" : "Details"} disabled={!game.onDetails} onClick={() => { close(); game.onDetails?.(); }}><MenuIcon name="externalLink" /></button>
        </HoverTooltip>
        <img src={game.cover || "/peligames.svg"} alt="" onError={event => { if (!event.currentTarget.src.endsWith("/peligames.svg")) event.currentTarget.src = "/peligames.svg"; }} />
        <div><strong>{game.name}</strong><small>{label(game)}</small></div>
        <button type="button" className="btn btn-secondary" disabled={Boolean(stopping) || game.stopDisabled || game.state === "stopping"} onClick={() => void stop(game)}><MenuIcon name="stop" />{game.state === "stopping" || stopping === game.id ? pt ? "Encerrando…" : "Stopping…" : pt ? "Parar" : "Stop"}</button>
      </article>)}
      {error && <p className="titlebar-running-error" role="alert">{error}</p>}
    </div>,document.body)}
  </>;
}

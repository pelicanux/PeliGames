import { MatrixSupporterBadge } from "./MatrixSupporterBadge";
import { invoke } from "@tauri-apps/api/core";
import { usePerformanceMode } from "./EffectsContext";
import { useEffect, useMemo, useRef, useState } from "react";
import { fetchGameCoverByName } from "../services/customCovers";
import { gameActivityName, useMatrixActivityPreferences } from "../services/matrixGameActivity";
import { useMatrixChat, displayName } from "../services/matrixChat";
import { readGameLibrary } from "../services/gameLibraryCache";
import { useI18n } from "../i18n/I18nContext";
import { MatrixPresenceBadge } from "./MatrixPresenceBadge";

const covers = new Map<string, Promise<string | null>>();
function Activity({ user, name, avatar, visible, supporter }: { supporter: boolean; user: string; name: string | null; avatar?: string | null; visible: boolean }) {
  const chat = useMatrixChat();
  const activity = useMatrixActivityPreferences();
  const { language } = useI18n();
  const ownCover = user === chat.userId && activity.name === name ? activity.cover : undefined;
  const [found, setFound] = useState<{name: string; url: string | null} | null>(null);
  const [avatarFailed, setAvatarFailed] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  useEffect(() => {
    if (!visible || !name || !activity.covers || ownCover) return;
    let active = true;
    const known = [...(readGameLibrary("all") || []), ...(readGameLibrary("own") || [])].find(game => game.name === name)?.cover_url;
    if (known) { setFound({ name, url: known }); return; }
    if (!covers.has(name)) covers.set(name, fetchGameCoverByName(name).catch(() => null));
    void covers.get(name)!.then(url => { if (active) setFound({ name, url }); });
    return () => { active = false; };
  }, [name, ownCover, activity.covers, visible]);
  const cover = ownCover || (found?.name === name ? found.url : undefined);
  const memberName = displayName(user, chat.names);
  return <div className="matrix-game-activity" data-member={user}>
    <div className="matrix-member-avatar">{avatar && !avatarFailed ? <img src={avatar} alt="" onError={() => setAvatarFailed(true)} /> : <span aria-hidden="true">{memberName.slice(0,2).toUpperCase()}</span>}<MatrixPresenceBadge status={chat.memberPresence[user]} pt={language === "pt"}/></div>
    {name && activity.covers && cover && failed !== cover && <img src={cover} alt="" onError={() => setFailed(cover)}/>}
    <div className="matrix-member-details"><div className="matrix-member-name"><strong title={user}>{memberName}</strong>{supporter && <MatrixSupporterBadge/>}</div>{name && <span title={name}>{language === "pt" ? "Jogando" : "Playing"} · {name}</span>}</div>
  </div>;
}
export function MatrixGameActivities({ users, supporters }: { users: Set<string>; supporters: Set<string> }) {
  const chat = useMatrixChat();
  const { language } = useI18n();
  const performance = usePerformanceMode();
  const entries = useMemo(() => [...users].filter(user => !chat.ignored.includes(user)).sort((a,b) => a === chat.userId ? -1 : b === chat.userId ? 1 : a.localeCompare(b)), [users, chat.ignored, chat.userId]);
  const viewport = useRef<HTMLDivElement>(null);
  const requested = useRef(new Set<string>());
  const mounted = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const [avatars, setAvatars] = useState<Record<string,string|null>>({});
  const [visible, setVisible] = useState<Set<string>>(new Set());
  useEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const wheel = (event: WheelEvent) => {
      if (element.scrollWidth <= element.clientWidth + 1 || event.ctrlKey) return;
      const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
      if (!delta) return;
      event.preventDefault(); event.stopPropagation();
      element.scrollLeft += delta * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? element.clientWidth : 1);
    };
    element.addEventListener("wheel", wheel, {passive:false});
    return () => element.removeEventListener("wheel", wheel);
  }, [entries]);
  useEffect(() => {
    const element = viewport.current;
    if (!element) return;
    // Fetch photos only for members entering the visible strip, never poll each member.
    const observer = new IntersectionObserver(items => {
      setVisible(previous => {
        const next = new Set(previous);
        items.forEach(item => { const user = (item.target as HTMLElement).dataset.member!; if (item.isIntersecting) next.add(user); else next.delete(user); });
        return next;
      });
      for (const item of items) {
        const user = (item.target as HTMLElement).dataset.member!;
        if (!item.isIntersecting || requested.current.has(user)) continue;
        requested.current.add(user);
        void invoke<string|null>("matrix_chat_avatar", {user}).then(avatar => { if (mounted.current) setAvatars(previous => ({...previous,[user]:avatar})); }).catch(() => {});
      }
    }, {root:element, rootMargin:"0px 40px", threshold:0});
    element.querySelectorAll("[data-member]").forEach(card => observer.observe(card));
    return () => observer.disconnect();
  }, [entries]);
  const move = (direction:number) => {
    const element = viewport.current;
    if (!element) return;
    const start = element.getBoundingClientRect().left;
    const positions = Array.from(element.querySelectorAll<HTMLElement>("[data-member]")).map(card => element.scrollLeft + card.getBoundingClientRect().left - start);
    const next = direction > 0 ? positions.find(position => position > element.scrollLeft + 2) ?? element.scrollWidth : [...positions].reverse().find(position => position < element.scrollLeft - 2) ?? 0;
    element.scrollTo({left:next,behavior:performance || window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth"});
  };
  if (!entries.length) return null;
  return <section className="matrix-members-carousel" aria-label={language === "pt" ? "Membros da sala" : "Room members"}>
    <div ref={viewport} className="matrix-game-activities" tabIndex={0} aria-label={language === "pt" ? "Lista de membros: use a barra de rolagem, a roda do mouse ou as setas do teclado" : "Member list: use the scrollbar, mouse wheel or keyboard arrows"}
      onKeyDown={event => { if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); move(event.key === "ArrowLeft" ? -1 : 1); } }}>
      {entries.map(user => <Activity key={user} user={user} name={gameActivityName(chat.presence[user] ?? "")} avatar={avatars[user]} supporter={supporters.has(user)} visible={visible.has(user)} />)}
    </div>
  </section>;
}

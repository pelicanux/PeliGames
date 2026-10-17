import { MatrixChannelList } from "./MatrixChannelList";
import { MatrixUserPanel, MatrixUserProfile } from "./MatrixUserPanel";
import { MatrixCallStage } from "./MatrixCallStage";
import { MatrixChannelSettings } from "./MatrixChannelSettings";
import type { ChannelEditTab } from "./MatrixChannelList";
import { useMatrixVoice } from "../services/matrixVoice";
import { OPEN_MATRIX_CALL_EVENT, takeCallViewRequest } from "../services/matrixScreenView";
import { MatrixSettings } from "./MatrixSettings";
import { MatrixSupporterBadge } from "./MatrixSupporterBadge";
import { usePerformanceMode } from "./EffectsContext";
import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ModalSurface } from "./ModalSurface";
import { MatrixIcon } from "./MatrixIcon";
import { MatrixGameActivities } from "./MatrixGameActivity";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { MATRIX_COMMUNITY_ID, MATRIX_PREMIUM_ROOM_ID } from "../services/matrixCommunity";
import { displayName, loadOlderMessages, mentionsMe, moderation, powerOf, refreshMatrixRoomMembers, refreshMatrixRooms, refreshMatrixStatus, reloadMatrixRooms, useMatrixChat, type ChatMessage } from "../services/matrixChat";
import { notifyError } from "../services/notifications";
import { GIF_LINK, matrixAnimatedImage, matrixAvatar, matrixGifMedia, matrixImage, matrixVideo, type GifMedia } from "../services/matrixMedia";
import { useDiscordBenefits } from "../theme/useDiscordBenefits";
import "./matrixChat.css";

/** `animated`: GIF or WebP, loaded whole so it moves (a thumbnail is a still frame). */
function ChatImage({ mxc, animated = false }: {mxc:string; animated?: boolean}) {
  const [image,setImage] = useState<string | null>(null);
  useEffect(()=>{let active=true;void (animated ? matrixAnimatedImage(mxc).then(value => value ?? matrixImage(mxc)) : matrixImage(mxc)).then(value=>{if(active)setImage(value);}).catch(()=>{});return()=>{active=false;};},[mxc, animated]);
  return image ? <img className="matrix-message-image" src={image} alt="Matrix"/> : null;
}
/** A short video, played like a GIF: looping, muted, inline (Discord's GIFs through a bridge). */
function ChatVideo({ mxc }: {mxc:string}) {
  const [video,setVideo] = useState<string | null>(null);
  useEffect(()=>{let active=true;void matrixVideo(mxc).then(value=>{if(active)setVideo(value);}).catch(()=>{});return()=>{active=false;};},[mxc]);
  return video ? <video className="matrix-message-image" src={video} autoPlay loop muted playsInline aria-label="Matrix" /> : null;
}
/** GIF links of a message shown as the GIF itself (Discord's GIF picker sends
 * Tenor or Giphy links through the bridge). `onlyLink` tells when the whole
 * message is one resolved GIF link, so its address can be hidden. */
function GifLinks({ text, onOnlyLink }: { text: string; onOnlyLink: (only: boolean) => void }) {
  const links = (text.match(LINK) ?? []).filter(link => GIF_LINK.test(link)).slice(0, 3);
  const [media, setMedia] = useState<(GifMedia | null)[]>([]);
  useEffect(() => {
    let active = true;
    void Promise.all(links.map(matrixGifMedia)).then(found => {
      if (!active) return;
      setMedia(found);
      onOnlyLink(links.length === 1 && Boolean(found[0]) && text.trim() === links[0]);
    });
    return () => { active = false; };
  }, [text]); // eslint-disable-line react-hooks/exhaustive-deps
  return <>{media.map((item, index) => item && (item.video
    ? <video key={index} className="matrix-message-image" src={item.src} autoPlay loop muted playsInline aria-label="GIF" />
    : <img key={index} className="matrix-message-image" src={item.src} alt="GIF" />))}</>;
}
/** The text of a message, plus the GIFs its links lead to. */
function MessageText({ text, onError }: { text: string; onError: (error: unknown) => void }) {
  const [onlyLink, setOnlyLink] = useState(false);
  return <>{!onlyLink && <Linked text={text} onError={onError}/>}{text.includes("https://") ? <GifLinks text={text} onOnlyLink={setOnlyLink}/> : null}</>;
}
const REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🔥", "🎮", "✅"];
/** Reactions of a message grouped by emoji, in order of first use. */
function groupReactions(list: { key: string; sender: string }[]) {
  const groups = new Map<string, string[]>();
  for (const item of list) groups.set(item.key, [...(groups.get(item.key) ?? []), item.sender]);
  return [...groups];
}
const userColor = (user: string) => { let hash = 0; for (const c of user) hash = (hash * 31 + c.charCodeAt(0)) >>> 0; return `hsl(${hash % 360} 48% 64%)`; };
const LINK = /(https?:\/\/[^\s<>"']+[^\s<>"'.,;:!?)\]])/g;
/** Message text with web links opened in the system browser; never HTML. */
function Linked({ text, onError }: { text: string; onError: (error: unknown) => void }) {
  const parts: ReactNode[] = text.split(LINK).map((part, index) => index % 2
    ? <a key={index} href={part} onClick={event => { event.preventDefault(); void openUrl(part).catch(onError); }}>{part}</a>
    : part);
  return <>{parts}</>;
}
export function MatrixChatPanel({ onClose }: { onClose: () => void }) {
  const screenClipId = `matrix-screen-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const screenShape = "M .10 .04 C .30 .013333 .70 .013333 .90 .04 C .953333 .047111 .965333 .08 .972 .14 C .992 .32 .992 .68 .972 .86 C .965333 .92 .953333 .952889 .90 .96 C .70 .986667 .30 .986667 .10 .96 C .046667 .952889 .034667 .92 .028 .86 C .008 .68 .008 .32 .028 .14 C .034667 .08 .046667 .047111 .10 .04 Z";
  const { language } = useI18n(); const pt = language === "pt";
  const performance = usePerformanceMode();
  const [powerOn, setPowerOn] = useState(true);
  useEffect(() => { const timer = window.setTimeout(() => setPowerOn(false), 1300); return () => window.clearTimeout(timer); }, []);
  const preview = "__PELI_UI_PREVIEW__" in window;
  const chat = useMatrixChat();
  const rooms = chat.roomList;
  const voice = useMatrixVoice();
  // The mini player asked for the call: the chat opens on the voice channel.
  const [room, setRoom] = useState(() => takeCallViewRequest() && voice.room ? voice.room : MATRIX_COMMUNITY_ID);
  // In the voice channel of the call, the call view replaces the chat until the chat is asked for.
  const [callChat, setCallChat] = useState(false);
  const selectRoom = (id: string) => { setRoom(id); setCallChat(false); };
  useEffect(() => {
    const open = () => { if (takeCallViewRequest() && voice.room) selectRoom(voice.room); };
    window.addEventListener(OPEN_MATRIX_CALL_EVENT, open);
    return () => window.removeEventListener(OPEN_MATRIX_CALL_EVENT, open);
  }, [voice.room]);
  const inCall = voice.phase !== "idle" && voice.room === room;
  const current = chat.rooms[room];
  const memberUsers = useMemo(() => new Set(current?.joined ? current.members.filter(user => user !== chat.userId) : []), [current?.joined, current?.members, chat.userId]);
  useEffect(() => {
    if (!chat.ready || !current?.joined) return;
    void refreshMatrixRoomMembers(room).catch(error => notifyError("Matrix", error));
  }, [room, chat.userId, chat.ready, current?.joined]);
  const { exclusiveEffects, benefitsChecked, benefit } = useDiscordBenefits();
  const [premium, setPremium] = useState<{user:string; phase:"checking" | "ready" | "error"} | null>(null);
  const [premiumAttempt, setPremiumAttempt] = useState(0);
  const premiumSelected = room === MATRIX_PREMIUM_ROOM_ID;
  // Being a member of the private room is the real authorization: the server let
  // this account in, and the Worker revalidates the Discord role daily, removing
  // whoever loses it. So a member opens the room at once, on every visit.
  const premiumJoined = Boolean(chat.rooms[MATRIX_PREMIUM_ROOM_ID]?.joined);
  const premiumReady = premiumJoined || (exclusiveEffects && premium?.user === chat.userId && premium.phase === "ready");
  const supporterUsers = useMemo(() => {
    const users = new Set<string>();
    if (premiumReady && chat.userId && (benefit === "youtube" || benefit === "booster")) users.add(chat.userId);
    // Visual example only; remote accounts require a shared server attestation.
    if (import.meta.env.DEV && preview) users.add("@pelicanux:matrix.org");
    return users;
  }, [premiumReady, chat.userId, benefit, preview]);
  useEffect(() => {
    let active = true;
    // The Worker is asked only to get an invitation, never again while a member.
    if (premiumJoined || !chat.userId || !chat.ready || !benefitsChecked || !exclusiveEffects) { setPremium(null); return; }
    const user = chat.userId;
    setPremium({user,phase:"checking"});
    void invoke<{granted:boolean;room_id:string;user_id:string}>("matrix_chat_premium_access").then(async grant => {
      if (!active) return;
      if (!grant.granted || grant.room_id !== MATRIX_PREMIUM_ROOM_ID || grant.user_id !== user) throw new Error("Autorização premium inválida.");
      await reloadMatrixRooms();
      if (active) setPremium({user,phase:"ready"});
    }).catch(error => { if (active) { setPremium({user,phase:"error"}); notifyError("Matrix", error); } });
    return () => { active = false; };
  }, [chat.userId, chat.ready, premiumJoined, benefitsChecked, exclusiveEffects, premiumAttempt]);
  const [avatars, setAvatars] = useState<Record<string,string | null>>({});
  const avatarLookups = useRef(new Set<string>());
  const [text, setText] = useState(""), [busy, setBusy] = useState(false);
  const [replyTo, setReplyTo] = useState<ChatMessage | null>(null);
  // Editing one of the user's own messages in the compose box.
  const [editing, setEditing] = useState<ChatMessage | null>(null);
  const [reacting, setReacting] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false), [avatarVersion, setAvatarVersion] = useState(0);
  // The channel editor opened from a channel's right-click menu.
  const [channelEdit, setChannelEdit] = useState<{ room: string; tab: ChannelEditTab } | null>(null);
  const settingsShown = useRef(false); settingsShown.current = settingsOpen;
  const editingShown = useRef(false); editingShown.current = Boolean(editing);
  const typingSent = useRef<{ room: string; at: number } | null>(null);
  const lastRead = useRef<Record<string, string>>({});
  // An action on one message that asks for a reason and confirmation first.
  type Pending = { id: string; kind: "report" | "redact" | "kick" | "ban" };
  const [pending, setPending] = useState<Pending | null>(null), [reason, setReason] = useState("");
  const ask = (id: string, kind: Pending["kind"]) => { setPending(pending?.id === id && pending.kind === kind ? null : { id, kind }); setReason(""); };
  const fileInput = useRef<HTMLInputElement>(null);
  const frame = useRef<HTMLDivElement>(null), log = useRef<HTMLDivElement>(null), input = useRef<HTMLTextAreaElement>(null);
  const atBottom = useRef(true), olderFrom = useRef<number | null>(null);
  const actionLock = useRef(false);
  const [refreshingRooms, setRefreshingRooms] = useState(false);
  const roomRefreshLock = useRef(false);
  // The parent passes a new function on every render; the effects below must not restart.
  const close = useRef(onClose); close.current = onClose;
  const fail = (e: unknown, fallback: string) => {
    const message = typeof e === "string" ? e : e instanceof Error ? e.message : fallback;
    notifyError("Matrix", message);
  };
  const refreshRooms = async () => {
    if (roomRefreshLock.current) return;
    roomRefreshLock.current = true; setRefreshingRooms(true);
    try { await refreshMatrixRooms(); await refreshMatrixRoomMembers(room); }
    catch (e) { fail(e, pt ? "Não foi possível atualizar as salas." : "Could not refresh rooms."); }
    finally { roomRefreshLock.current = false; setRefreshingRooms(false); }
  };
  useEffect(() => {
    if (chat.userId) void refreshRooms();
  }, [chat.userId]);
  useEffect(() => {
    void refreshMatrixStatus();
    const oldFocus = document.activeElement; frame.current?.querySelector<HTMLButtonElement>("button")?.focus();
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === "Escape" && event.target instanceof Element && event.target.closest('[role="listbox"]')) return;
      if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); if (settingsShown.current) setSettingsOpen(false); else if (editingShown.current) { setEditing(null); setText(""); } else close.current(); }
      if (event.key === "Tab") {
        const controls = frame.current?.querySelectorAll<HTMLElement>("button:not(:disabled),textarea,input:not([type=file]),a[href]");
        if (!controls?.length) return;
        const first = controls[0], last = controls[controls.length-1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    };
    window.addEventListener("keydown",keyboard,true);
    return () => { window.removeEventListener("keydown",keyboard,true); if (oldFocus instanceof HTMLElement && oldFocus.isConnected) oldFocus.focus(); };
  }, []);
  useEffect(() => { if (chat.error) notifyError("Matrix", chat.error); }, [chat.error]);
  // Keep the selected room valid once the community's room list arrives.
  useEffect(() => { if (rooms.length && !rooms.some(item => item.room_id === room)) setRoom(rooms[0].room_id); }, [rooms, room]);
  useEffect(() => { setReplyTo(null); setPending(null); setEditing(null); setReacting(null); atBottom.current = true; }, [room]);
  const messages = (premiumSelected && !premiumReady ? [] : current?.messages ?? []).filter(message => !chat.ignored.includes(message.sender));
  useEffect(() => {
    for (const sender of new Set(messages.map(message => message.sender))) {
      if (avatarLookups.current.has(sender)) continue;
      avatarLookups.current.add(sender);
      void matrixAvatar(sender).then(avatar=>{if (frame.current?.isConnected) setAvatars(previous=>({...previous,[sender]:avatar}));}).catch(()=>{});
    }
  }, [messages]);
  // Reading the open room clears its unread count, on every device (private receipt).
  const lastMessage = messages.length ? messages[messages.length - 1].id : null;
  useEffect(() => {
    if (preview || !lastMessage || !current?.joined || (premiumSelected && !premiumReady) || lastRead.current[room] === lastMessage) return;
    if (!current.unread && lastRead.current[room] !== undefined) { lastRead.current[room] = lastMessage; return; }
    lastRead.current[room] = lastMessage;
    void invoke("matrix_chat_read", { room, event: lastMessage }).catch(() => { delete lastRead.current[room]; });
  }, [room, lastMessage, current?.joined, current?.unread, premiumReady]);
  // New messages follow the end only while the reader is already there; older
  // history keeps the message in view where it was.
  useLayoutEffect(() => {
    const element = log.current; if (!element) return;
    if (olderFrom.current !== null) { element.scrollTop += element.scrollHeight - olderFrom.current; olderFrom.current = null; }
    else if (atBottom.current) element.scrollTop = element.scrollHeight;
  }, [messages.length, room]);
  const run = async (command: string, args?: Record<string, unknown>) => {
    if (actionLock.current) return false; actionLock.current = true; setBusy(true);
    try { await invoke(command,args); return true; }
    catch (e) { fail(e, pt ? "Não foi possível concluir a operação." : "Could not complete the operation."); return false; }
    finally { actionLock.current = false; setBusy(false); }
  };
  // "Typing…" for the others: sent at most every 4 s, ended when the box empties or the message goes.
  const typing = (active: boolean) => {
    if (preview || !current?.joined) return;
    const now = Date.now(), sent = typingSent.current;
    if (active && sent?.room === room && now - sent.at < 4000) return;
    if (!active && !sent) return;
    typingSent.current = active ? { room, at: now } : null;
    void invoke("matrix_chat_typing", { room: active ? room : sent?.room ?? room, typing: active }).catch(() => {});
  };
  const send = async () => {
    const value = text.trim(); if (!value || busy || (premiumSelected && !premiumReady)) return;
    typing(false);
    if (editing) {
      const target = editing;
      setText(""); setEditing(null);
      if (value !== target.body && !await run("matrix_chat_edit", { room, target: target.id, text: value })) { setText(current => current || value); setEditing(current => current ?? target); }
      return;
    }
    const emote = /^\/me\s+/i.test(value);
    atBottom.current = true;
    // The box clears right away, as in other chats; a failed send gives the text back.
    const reply = replyTo;
    setText(""); setReplyTo(null);
    if (!await run("matrix_chat_send", { room, text: emote ? value.replace(/^\/me\s+/i, "") : value, replyTo: reply?.id ?? null, replySender: reply?.sender ?? null, emote })) {
      setText(current => current || text); setReplyTo(current => current ?? reply);
    }
  };
  const disconnect = async () => {
    if (actionLock.current) return; actionLock.current = true; setBusy(true);
    try {
      // Offline, the account still leaves this computer; the server keeps the session until it is ended in Element.
      if (!await invoke<boolean>("disconnect_matrix_chat")) fail(null, pt ? "Conta removida deste computador, mas sem conexão para encerrar a sessão no Matrix. Encerre-a no Element, em Configurações → Sessões." : "Account removed from this computer, but there was no connection to end the Matrix session. End it in Element, under Settings → Sessions.");
    } catch (e) { fail(e, pt ? "Não foi possível desconectar." : "Could not disconnect."); }
    finally { actionLock.current = false; setBusy(false); void refreshMatrixStatus(); }
  };
  // Reacting again with the same emoji removes the reaction (its event is redacted).
  const react = async (message: ChatMessage, key: string) => {
    setReacting(null);
    const mine = current?.reactions[message.id]?.find(item => item.key === key && item.sender === chat.userId);
    if (mine) await run("matrix_chat_moderate", { room, action: "redact", target: mine.id, reason: "" });
    else await run("matrix_chat_react", { room, target: message.id, key });
  };
  const join = async (target: string) => { if (await run("matrix_chat_join", { room: target })) await reloadMatrixRooms(); };
  const older = async () => {
    olderFrom.current = log.current?.scrollHeight ?? null;
    try { await loadOlderMessages(room); } catch (e) { olderFrom.current = null; fail(e, pt ? "Não foi possível carregar mensagens anteriores." : "Could not load older messages."); }
  };
  const upload = async (file: File) => {
    if (premiumSelected && !premiumReady) return;
    if (file.size > 5*1024*1024) { fail(null, pt ? "O arquivo deve ter até 5 MB." : "File must be at most 5 MB."); return; }
    const mime = /\.(txt|log)$/i.test(file.name) ? "text/plain" : file.type;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      atBottom.current = true;
      if (actionLock.current) return; actionLock.current = true; setBusy(true);
      await invoke("matrix_chat_upload", bytes, { headers: { "x-matrix-room": encodeURIComponent(room), "x-matrix-name": encodeURIComponent(file.name), "x-matrix-mime": encodeURIComponent(mime) } });
    } catch (e) { fail(e, pt ? "Não foi possível enviar o arquivo." : "Could not send the file."); }
    finally { actionLock.current = false; setBusy(false); }
  };
  const confirm = async (message: ChatMessage, kind: Pending["kind"]) => {
    const done = kind === "report"
      ? await run("matrix_chat_report", { room, event: message.id, reason })
      : await run("matrix_chat_moderate", { room, action: kind, target: kind === "redact" ? message.id : message.sender, reason });
    if (done) setPending(null);
  };
  const actionText = {
    report: { title: pt ? "Denunciar aos moderadores" : "Report to moderators", button: pt ? "Enviar denúncia" : "Send report" },
    redact: { title: pt ? "Apagar esta mensagem para todos" : "Remove this message for everyone", button: pt ? "Apagar" : "Remove" },
    kick: { title: pt ? "Remover a pessoa da sala (ela pode voltar)" : "Remove the person from the room (they can come back)", button: pt ? "Expulsar" : "Kick" },
    ban: { title: pt ? "Banir a pessoa desta sala" : "Ban the person from this room", button: pt ? "Banir" : "Ban" },
  };
  const byId = new Map(messages.map(message => [message.id, message]));
  const communityJoined = chat.rooms[MATRIX_COMMUNITY_ID]?.joined;
  const joinedRooms = useMemo(() => new Set(Object.entries(chat.rooms).filter(([, value]) => value.joined).map(([id]) => id)), [chat.rooms]);
  // Adding channels to the community needs the space's m.space.child power (the server checks again).
  const communityPower = chat.rooms[MATRIX_COMMUNITY_ID]?.power;
  const canCreateChannels = Boolean(communityJoined && chat.userId && communityPower && powerOf(communityPower, chat.userId) >= (communityPower.space_child ?? 50));
  const unreadRooms = useMemo(() => Object.fromEntries(Object.entries(chat.rooms).map(([id, value]) => [id, value.unread])), [chat.rooms]);
  const typingNames = (current?.typing ?? []).filter(user => !chat.ignored.includes(user)).map(user => displayName(user, chat.names));
  const selectedRoom = rooms.find(item=>item.room_id === room);
  const roomName = selectedRoom?.name || (room === MATRIX_COMMUNITY_ID ? "Pelicanux" : pt ? "Sala" : "Room");
  const roomTopic = selectedRoom?.topic?.trim();
  return createPortal(<div className="modal-overlay matrix-chat-overlay" onMouseDown={event => { if (event.target === event.currentTarget) close.current(); }}>
    <ModalSurface ref={frame} className="modal-content dialog-glass matrix-chat" role="dialog" aria-modal="true" aria-label={`Matrix · ${roomName}`} onDismiss={() => close.current()}>
      <div className="matrix-terminal-frame" style={{clipPath:`url(#${screenClipId})`}}>
      <svg width="0" height="0" className="matrix-crt-definitions" aria-hidden="true" focusable="false"><defs><clipPath id={screenClipId} clipPathUnits="objectBoundingBox"><path d={screenShape}/></clipPath></defs></svg>
      <svg className="matrix-crt-outline" viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true" focusable="false"><path d={screenShape} fill="none" vectorEffect="non-scaling-stroke"/></svg>
      {powerOn && !performance && <div className="matrix-crt-power-beam" aria-hidden="true" />}
      {powerOn && !performance && <div className="matrix-crt-artifacts" aria-hidden="true" onAnimationEnd={event => { if (event.animationName === "matrix-crt-artifact-burst") setPowerOn(false); }} />}
      <div className={`matrix-crt-screen${powerOn && !performance ? " matrix-crt-power-on" : ""}`}>
      <div className="matrix-terminal-status"><div className="matrix-terminal-session">{chat.userId ? <MatrixUserProfile pt={pt} me={chat.userId} names={chat.names} avatarVersion={avatarVersion} connected={chat.ready && !chat.error}/> : <><span className="matrix-terminal-dot"/><span>{pt ? "SESSÃO DESCONECTADA" : "DISCONNECTED SESSION"}</span></>}</div><div className="matrix-terminal-activity"><MatrixGameActivities key={`${chat.userId}:${room}`} users={memberUsers} supporters={supporterUsers}/></div><small>{preview ? pt ? "DEMONSTRAÇÃO · SEM LOGIN OU ENVIO REAL" : "DEMO · NO REAL SIGN-IN OR MESSAGES" : pt ? "CHAT DA COMUNIDADE" : "COMMUNITY CHAT"}</small></div>
      {!chat.userId ? <div className="matrix-terminal-welcome"><MatrixIcon/><h3>{pt ? "Conecte-se à comunidade" : "Connect to the community"}</h3><p>{pt ? "Converse sobre jogos, mods e Linux num chat com cara de terminal. Entre com sua conta Matrix pelo navegador." : "Talk about games, mods and Linux in a terminal-style chat. Sign in with your Matrix account through the browser."}</p><button className="btn btn-primary" disabled={busy || chat.pending} onClick={() => void run("connect_matrix_chat").then(() => refreshMatrixStatus())}><MatrixIcon/>{chat.pending ? pt ? "Aguardando login…" : "Waiting for sign-in…" : pt ? "Entrar com Matrix" : "Sign in with Matrix"}</button><p>{chat.pending ? pt ? "Autorize no navegador e volte aqui. O login expira em três minutos." : "Authorize in your browser and return here. Sign-in expires in three minutes." : pt ? "Sua senha não fica salva no launcher." : "Your password is not saved in the launcher."}</p></div> : <div className="matrix-terminal-layout">
        <aside className="matrix-terminal-rooms">
          {chat.ready && !communityJoined && <button className="matrix-terminal-join" disabled={busy} onClick={() => void join(MATRIX_COMMUNITY_ID)}>{pt ? "+ Entrar na comunidade" : "+ Join the community"}</button>}
          {preview && <small className="matrix-rooms-demo">{pt ? "Lista simulada na preview" : "Simulated preview rooms"}</small>}
          <div className="matrix-rooms-scroll">
          <MatrixChannelList pt={pt} rooms={rooms} room={room} onSelect={selectRoom} me={chat.userId} names={chat.names} joined={joinedRooms} ready={chat.ready} unread={unreadRooms}
            canCreate={canCreateChannels} locked={id => id === MATRIX_PREMIUM_ROOM_ID && !premiumReady} lockLabel={id => id === MATRIX_PREMIUM_ROOM_ID && !premiumReady ? " 🔒" : ""}
            onJoinRoom={async id => { if (!await run("matrix_chat_join", { room: id })) return false; await reloadMatrixRooms(); return true; }}
            onError={fail} refreshing={refreshingRooms} onRefresh={() => void refreshRooms()} permanent={id => id === MATRIX_COMMUNITY_ID || id === MATRIX_PREMIUM_ROOM_ID}
            onOpenChat={id => { setRoom(id); setCallChat(true); }} onEdit={(id, tab) => setChannelEdit({ room: id, tab })}
            canEdit={id => canCreateChannels || Boolean(chat.userId && chat.rooms[id]?.power && powerOf(chat.rooms[id]!.power, chat.userId) >= 50)}/>
          {chat.ignored.length > 0 && <><h3>{pt ? "# IGNORADOS" : "# IGNORED"}</h3>{chat.ignored.map(user => <button key={user} className="matrix-terminal-ignored" disabled={busy} title={pt ? "Voltar a mostrar" : "Show again"} onClick={() => void run("matrix_chat_ignore", { user, ignore: false })}>{displayName(user, chat.names)} ✕</button>)}</>}
          </div>
          <MatrixUserPanel pt={pt} onSettings={() => setSettingsOpen(true)}/>
        </aside>
        {inCall && !callChat ? <MatrixCallStage pt={pt} me={chat.userId} roomName={roomName} videoHidden={settingsOpen || Boolean(channelEdit)} onShowChat={() => setCallChat(true)}/> :
        <section className="matrix-terminal-room"><header><div className="matrix-room-heading"><strong title={roomName}># {roomName}</strong>{roomTopic && <small className="matrix-room-topic" title={roomTopic}>{roomTopic}</small>}</div>{inCall && <button type="button" className="btn btn-secondary" onClick={() => setCallChat(false)}>{pt ? "Ver chamada" : "Show call"}</button>}{!premiumSelected && chat.ready && !current?.joined && <button className="btn btn-secondary" disabled={busy} onClick={() => void join(room)}>{pt ? "Entrar na sala" : "Join room"}</button>}</header>
          {premiumSelected && !premiumReady ? <div className="matrix-premium-gate" role="status">
            <h3>{roomName}</h3>
            <p>{!benefitsChecked || premium?.phase === "checking" ? pt ? "Verificando o Discord e liberando sua conta Matrix…" : "Checking Discord and granting Matrix access…" : !exclusiveEffects ? pt ? "Exclusivo para membros do YouTube e impulsionadores do servidor no Discord. Entre com Discord em Preferências → Conexões para liberar." : "For YouTube members and Discord server boosters. Sign in with Discord in Preferences → Connections to unlock." : (pt ? "Confirme sua conexão para acessar esta sala." : "Confirm your connection to access this room.")}</p>
            {exclusiveEffects && premium?.phase !== "checking" && <button type="button" className="btn btn-secondary" onClick={() => setPremiumAttempt(value=>value+1)}>{pt ? "Tentar novamente" : "Try again"}</button>}
          </div> : <>
          <div ref={log} className="matrix-terminal-messages" role="log" aria-live="polite" aria-relevant="additions" onScroll={event => { const element = event.currentTarget; atBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80; }}>
            {current?.encrypted ? <p>{pt ? "Esta sala usa criptografia. Abra o Element para conversar com segurança." : "This room uses encryption. Open Element to chat securely."}</p> : <>
              {current?.prevBatch && messages.length > 0 && <button className="matrix-terminal-older" disabled={current.loadingOlder} onClick={() => void older()}>{current.loadingOlder ? pt ? "> carregando…" : "> loading…" : pt ? "> carregar mensagens anteriores" : "> load older messages"}</button>}
              {messages.length ? messages.map(message => {
                const name = displayName(message.sender, chat.names), color = userColor(message.sender), own = message.sender === chat.userId;
                const replied = message.reply_to ? byId.get(message.reply_to) : undefined;
                const allowed = chat.userId ? moderation(current?.power, chat.userId, message.sender) : { redact: false, kick: false, ban: false };
                return <article key={message.id} className={`matrix-terminal-message${chat.userId && mentionsMe(message, chat.userId, chat.names) ? " is-mention" : ""}${message.msgtype === "m.notice" ? " is-notice" : ""}`}>
                  <div className="matrix-terminal-avatar" style={{color,borderColor:color}} aria-hidden="true">{avatars[message.sender] ? <img src={avatars[message.sender]!} alt=""/> : name.slice(0,2).toUpperCase()}</div>
                  <div>
                    <header><strong style={{color}} title={message.sender}>{name}</strong>{supporterUsers.has(message.sender) && <MatrixSupporterBadge/>}<time>{new Date(message.ts).toLocaleTimeString(pt ? "pt-BR" : "en-US",{hour:"2-digit",minute:"2-digit"})}</time>{message.edited && <small>{pt ? "(editada)" : "(edited)"}</small>}
                      <span className="matrix-message-actions">
                        <button type="button" onClick={() => { setEditing(null); setReplyTo(message); input.current?.focus(); }}>{pt ? "responder" : "reply"}</button>
                        <button type="button" aria-expanded={reacting === message.id} onClick={() => setReacting(reacting === message.id ? null : message.id)}>{pt ? "reagir" : "react"}</button>
                        {own && (message.msgtype === "m.text" || message.msgtype === "m.emote") && <button type="button" onClick={() => { setReplyTo(null); setEditing(message); setText(message.body); input.current?.focus(); }}>{pt ? "editar" : "edit"}</button>}
                        {!own && <button type="button" onClick={() => ask(message.id, "report")}>{pt ? "denunciar" : "report"}</button>}
                        {!own && <button type="button" disabled={busy} onClick={() => void run("matrix_chat_ignore", { user: message.sender, ignore: true })}>{pt ? "ignorar" : "ignore"}</button>}
                        {allowed.redact && <button type="button" onClick={() => ask(message.id, "redact")}>{pt ? "apagar" : "remove"}</button>}
                        {allowed.kick && <button type="button" className="is-moderation" onClick={() => ask(message.id, "kick")}>{pt ? "expulsar" : "kick"}</button>}
                        {allowed.ban && <button type="button" className="is-moderation" onClick={() => ask(message.id, "ban")}>{pt ? "banir" : "ban"}</button>}
                      </span>
                    </header>
                    {message.reply_to && <p className="matrix-message-reply">↳ {replied ? <><b>{displayName(replied.sender, chat.names)}</b>: {replied.body.slice(0, 120)}</> : pt ? "resposta a uma mensagem anterior" : "reply to an earlier message"}</p>}
                    <p>{message.msgtype === "m.emote" ? <>* <b style={{color}}>{name}</b> </> : null}{message.msgtype === "m.file" ? `[${pt ? "arquivo" : "file"}] ` : null}{message.msgtype === "m.image" ? null : <MessageText text={message.body} onError={e => fail(e, pt ? "Não foi possível abrir o link." : "Could not open the link.")}/>}</p>
                    {message.msgtype === "m.image" && message.url && <ChatImage mxc={message.url} animated={message.mimetype === "image/gif" || message.mimetype === "image/webp"}/>}
                    {message.msgtype === "m.video" && message.url && <ChatVideo mxc={message.url}/>}
                    {reacting === message.id && <div className="matrix-reaction-picker" role="group" aria-label={pt ? "Escolher reação" : "Choose a reaction"}>{REACTIONS.map(key => <button key={key} type="button" disabled={busy} onClick={() => void react(message, key)}>{key}</button>)}</div>}
                    {(current?.reactions[message.id]?.length ?? 0) > 0 && <div className="matrix-reactions">{groupReactions(current!.reactions[message.id]).map(([key, senders]) => {
                      const mine = Boolean(chat.userId && senders.includes(chat.userId));
                      return <button key={key} type="button" className={mine ? "is-mine" : undefined} aria-pressed={mine} disabled={busy} title={senders.map(user => displayName(user, chat.names)).join(", ")} onClick={() => void react(message, key)}>{key} {senders.length}</button>;
                    })}</div>}
                    {pending?.id === message.id && <form className={`matrix-message-report${pending.kind === "kick" || pending.kind === "ban" ? " is-moderation" : ""}`} onSubmit={event => { event.preventDefault(); void confirm(message, pending.kind); }}>
                      <strong>{actionText[pending.kind].title}{pending.kind === "kick" || pending.kind === "ban" ? `: ${name}` : ""}</strong>
                      <input value={reason} maxLength={pending.kind === "report" ? 1000 : 500} autoFocus onChange={event => setReason(event.target.value)} placeholder={pt ? "Motivo (opcional)" : "Reason (optional)"}/>
                      <button type="submit" className="btn btn-secondary" disabled={busy}>{actionText[pending.kind].button}</button>
                      <button type="button" className="btn btn-secondary" onClick={() => setPending(null)}>{pt ? "Cancelar" : "Cancel"}</button>
                    </form>}
                  </div>
                </article>;
              }) : <p className="matrix-terminal-empty">{!chat.ready ? pt ? "> conectando…" : "> connecting…" : current?.joined ? pt ? "> Nenhuma mensagem ainda. Diga oi!" : "> No messages yet. Say hi!" : pt ? "> Entre na sala para ver as mensagens." : "> Join the room to see the messages."}</p>}
            </>}
          </div>
          <form className="matrix-terminal-compose" onSubmit={event=>{event.preventDefault();void send();}}>
            {typingNames.length > 0 && <p className="matrix-typing" aria-live="polite">{typingNames.length > 3 ? (pt ? "> várias pessoas estão digitando…" : "> several people are typing…") : `> ${typingNames.join(", ")} ${typingNames.length > 1 ? (pt ? "estão digitando…" : "are typing…") : (pt ? "está digitando…" : "is typing…")}`}</p>}
            {editing ? <p className="matrix-compose-reply">✎ {pt ? "editando sua mensagem · Esc cancela" : "editing your message · Esc cancels"} <button type="button" onClick={() => { setEditing(null); setText(""); }} aria-label={pt ? "Cancelar edição" : "Cancel edit"}>✕</button></p>
              : replyTo ? <p className="matrix-compose-reply">↳ {pt ? "respondendo a" : "replying to"} <b>{displayName(replyTo.sender, chat.names)}</b>: {replyTo.body.slice(0, 80)} <button type="button" onClick={() => setReplyTo(null)} aria-label={pt ? "Cancelar resposta" : "Cancel reply"}>✕</button></p>
              : <label htmlFor="matrix-message">{pt ? "> MENSAGEM · Enter envia, Shift+Enter quebra a linha, /me para ação" : "> MESSAGE · Enter sends, Shift+Enter new line, /me for actions"}</label>}
            <div><input ref={fileInput} type="file" hidden accept="image/png,image/jpeg,image/gif,image/webp,.txt,.log" onChange={event=>{const file = event.target.files?.[0]; event.target.value = ""; if (file) void upload(file);}}/>
              <button type="button" className="btn btn-secondary matrix-attach-button" disabled={busy || current?.encrypted || !current?.joined} onClick={()=>fileInput.current?.click()} aria-label={pt ? "Enviar imagem ou log" : "Send image or log"} title={pt ? "Enviar imagem ou log" : "Send image or log"}><MenuIcon name="plus"/></button>
              <textarea ref={input} id="matrix-message" value={text} maxLength={8000} disabled={current?.encrypted || !current?.joined} onChange={event=>{setText(event.target.value); typing(Boolean(event.target.value.trim()));}} onKeyDown={event => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void send(); } }} placeholder={pt ? "Digite sua mensagem…" : "Type your message…"}/>
              <button className="matrix-send-button" aria-label={pt ? "Enviar mensagem · Enter" : "Send message · Enter"} disabled={busy || current?.encrypted || !current?.joined || !text.trim()} type="submit">
                <svg className="matrix-enter-key" width="72" height="82" viewBox="0 0 120 140" fill="none" aria-hidden="true" focusable="false">
                  <g stroke="currentColor" strokeWidth="2" strokeLinejoin="round" strokeLinecap="round">
                    <path d="M20 8Q61 11 102 8L114 26V125Q114 132 107 132H43Q36 132 36 125V70H14Q8 70 8 64V27L20 8Z"/>
                    <path d="M20 8V51L48 55V114Q75 122 102 114V8M8 70L20 51M36 70L48 55M36 128L48 114M102 114L112 129"/>
                  </g>
                  <text x="68" y="43" textAnchor="middle" fill="currentColor">Enter</text>
                </svg>
              </button></div>
          </form>
          </>}
        </section>}
      </div>}
      {channelEdit && chat.userId && <MatrixChannelSettings key={channelEdit.room} pt={pt} room={channelEdit.room} names={chat.names} initialTab={channelEdit.tab} onClose={() => setChannelEdit(null)} onError={fail} onSaved={() => void reloadMatrixRooms()}/>}
      {settingsOpen && chat.userId && <MatrixSettings pt={pt} me={chat.userId} names={chat.names} onClose={() => setSettingsOpen(false)} onError={fail}
        onDisconnect={() => { setSettingsOpen(false); void disconnect(); }} onAvatarChanged={() => { setAvatarVersion(value => value + 1); setAvatars(previous => { const next = { ...previous }; delete next[chat.userId!]; return next; }); avatarLookups.current.delete(chat.userId!); }} avatarVersion={avatarVersion}/>}
      <footer className="matrix-terminal-footer"><h2 className="matrix-terminal-brand"><MatrixIcon/><span>Matrix</span><span className="matrix-terminal-tag">// Pelicanux</span></h2></footer>
      </div>
      </div>
    </ModalSurface>
  </div>,document.body);
}

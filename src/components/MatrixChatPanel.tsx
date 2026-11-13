import {MatrixEmojiPicker} from "./MatrixEmojiPicker";
import {useAutosizeTextarea} from "../hooks/useAutosizeTextarea";
import { ConnectionStorageChoice } from "./ConnectionStorageChoice";
import { MatrixChannelList } from "./MatrixChannelList";
import { MatrixUserPanel, MatrixUserProfile } from "./MatrixUserPanel";
import { MatrixCallStage } from "./MatrixCallStage";
import { MatrixChannelMenu } from "./MatrixChannelMenu";
import { MatrixUserMenu, type UserMenuTarget } from "./MatrixUserMenu";
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
import { displayName, loadOlderMessages, markMatrixRoomRead, mentionsMe, moderation, powerOf, refreshMatrixRoomMembers, refreshMatrixRooms, disconnectMatrixChat, refreshMatrixStatus, reloadMatrixRooms, setMatrixUserBlocked, useMatrixChat, type ChatMessage } from "../services/matrixChat";
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
  const {t: tx, language} = useI18n();

  const screenClipId = `matrix-screen-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const screenShape = "M .10 .04 C .30 .013333 .70 .013333 .90 .04 C .953333 .047111 .965333 .08 .972 .14 C .992 .32 .992 .68 .972 .86 C .965333 .92 .953333 .952889 .90 .96 C .70 .986667 .30 .986667 .10 .96 C .046667 .952889 .034667 .92 .028 .86 C .008 .68 .008 .32 .028 .14 C .034667 .08 .046667 .047111 .10 .04 Z";

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
  const memberUsers = useMemo(() => new Set(current?.joined ? [...current.members,...(chat.userId?[chat.userId]:[])] : []), [current?.joined, current?.members, chat.userId]);
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
  // The "⋯" menu of a message (edit, report, block, moderation) and a profile opened from it.
  const [messageMenu, setMessageMenu] = useState<{ message: ChatMessage; x: number; y: number } | null>(null);
  const [profileOf, setProfileOf] = useState<UserMenuTarget | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false), [avatarVersion, setAvatarVersion] = useState(0);
  // The channel editor opened from a channel's right-click menu.
  const [channelEdit, setChannelEdit] = useState<{ room: string; tab: ChannelEditTab } | null>(null);
  const settingsShown = useRef(false); settingsShown.current = settingsOpen;
  const editingShown = useRef(false); editingShown.current = Boolean(editing);
  const typingSent = useRef<{ room: string; at: number } | null>(null);
  // An action on one message that asks for a reason and confirmation first.
  type Pending = { id: string; kind: "report" | "redact" | "kick" | "ban" };
  const [pending, setPending] = useState<Pending | null>(null), [reason, setReason] = useState("");
  const ask = (id: string, kind: Pending["kind"]) => { setPending(pending?.id === id && pending.kind === kind ? null : { id, kind }); setReason(""); };
  const fileInput = useRef<HTMLInputElement>(null);
  const frame = useRef<HTMLDivElement>(null), log = useRef<HTMLDivElement>(null), input = useRef<HTMLTextAreaElement>(null);
  useAutosizeTextarea(input,text,Boolean(chat.userId)&&(!inCall||callChat)&&(!premiumSelected||premiumReady),4);
  const insertEmoji=(emoji:string)=>{
    const field=input.current;
    if(!field)return;
    const start=field.selectionStart,end=field.selectionEnd;
    const next=text.slice(0,start)+emoji+text.slice(end);
    if(next.length>field.maxLength)return;
    setText(next);typing(true);
    requestAnimationFrame(()=>{if(input.current!==field)return;field.focus();field.setSelectionRange(start+emoji.length,start+emoji.length)});
  };
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
    catch (e) { fail(e, tx("ui-matrix-chat-panel", "could_not_refresh_rooms")); }
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
  // Persist every displayed tail; unread counts can arrive after the message.
  const lastMessage = current?.messages[current.messages.length - 1]?.id ?? null;
  useEffect(() => {
    if (!lastMessage || !current?.joined || (premiumSelected && !premiumReady) || (inCall && !callChat)) return;
    const read = () => {
      if (document.visibilityState === "hidden" || !document.hasFocus()) return;
      void markMatrixRoomRead(room, lastMessage).catch(() => {
        // A failed receipt stays retryable when the room is reopened or updated.
      });
    };
    read();
    window.addEventListener("focus", read);
    document.addEventListener("visibilitychange", read);
    return () => { window.removeEventListener("focus", read); document.removeEventListener("visibilitychange", read); };
  }, [room, lastMessage, current?.joined, current?.unread, premiumReady, premiumSelected, inCall, callChat]);
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
    catch (e) { fail(e, tx("ui-matrix-chat-panel", "could_not_complete_the_operation")); return false; }
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
      if (!await disconnectMatrixChat()) fail(null, tx("ui-matrix-chat-panel", "account_removed_from_this_computer_but_there_was_no_connection_to_end_the_matrix_session"));
    } catch (e) { fail(e, tx("ui-matrix-chat-panel", "could_not_disconnect")); }
    finally { actionLock.current = false; setBusy(false); }
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
    try { await loadOlderMessages(room); } catch (e) { olderFrom.current = null; fail(e, tx("ui-matrix-chat-panel", "could_not_load_older_messages")); }
  };
  const upload = async (file: File) => {
    if (premiumSelected && !premiumReady) return;
    if (file.size > 5*1024*1024) { fail(null, tx("ui-matrix-chat-panel", "file_must_be_at_most_5_mb")); return; }
    const mime = /\.(txt|log)$/i.test(file.name) ? "text/plain" : file.type;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      atBottom.current = true;
      if (actionLock.current) return; actionLock.current = true; setBusy(true);
      await invoke("matrix_chat_upload", bytes, { headers: { "x-matrix-room": encodeURIComponent(room), "x-matrix-name": encodeURIComponent(file.name), "x-matrix-mime": encodeURIComponent(mime) } });
    } catch (e) { fail(e, tx("ui-matrix-chat-panel", "could_not_send_the_file")); }
    finally { actionLock.current = false; setBusy(false); }
  };
  const confirm = async (message: ChatMessage, kind: Pending["kind"]) => {
    const done = kind === "report"
      ? await run("matrix_chat_report", { room, event: message.id, reason })
      : await run("matrix_chat_moderate", { room, action: kind, target: kind === "redact" ? message.id : message.sender, reason });
    if (done) setPending(null);
  };
  const actionText = {
    report: { title: tx("ui-matrix-chat-panel", "report_to_moderators"), button: tx("ui-matrix-chat-panel", "send_report") },
    redact: { title: tx("ui-matrix-chat-panel", "remove_this_message_for_everyone"), button: tx("ui-matrix-chat-panel", "remove") },
    kick: { title: tx("ui-matrix-chat-panel", "remove_the_person_from_the_room_they_can_come_back"), button: tx("ui-matrix-chat-panel", "kick") },
    ban: { title: tx("ui-matrix-chat-panel", "ban_the_person_from_this_room"), button: tx("ui-matrix-chat-panel", "ban") },
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
  const roomName = selectedRoom?.name || (room === MATRIX_COMMUNITY_ID ? "Pelicanux" : tx("ui-matrix-chat-panel", "room"));
  const roomTopic = selectedRoom?.topic?.trim();
  return createPortal(<div className="modal-overlay matrix-chat-overlay" onMouseDown={event => { if (event.target === event.currentTarget) close.current(); }}>
    <ModalSurface ref={frame} className="modal-content dialog-glass matrix-chat" role="dialog" aria-modal="true" aria-label={`Matrix · ${roomName}`} onDismiss={() => close.current()}>
      <div className="matrix-terminal-frame" style={{clipPath:`url(#${screenClipId})`}}>
      <svg width="0" height="0" className="matrix-crt-definitions" aria-hidden="true" focusable="false"><defs><clipPath id={screenClipId} clipPathUnits="objectBoundingBox"><path d={screenShape}/></clipPath></defs></svg>
      <svg className="matrix-crt-outline" viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true" focusable="false"><path d={screenShape} fill="none" vectorEffect="non-scaling-stroke"/></svg>
      {powerOn && !performance && <div className="matrix-crt-power-beam" aria-hidden="true" />}
      {powerOn && !performance && <div className="matrix-crt-artifacts" aria-hidden="true" onAnimationEnd={event => { if (event.animationName === "matrix-crt-artifact-burst") setPowerOn(false); }} />}
      <div className={`matrix-crt-screen${powerOn && !performance ? " matrix-crt-power-on" : ""}`}>
      <div className="matrix-terminal-status"><div className="matrix-terminal-session">{chat.userId ? <MatrixUserProfile me={chat.userId} names={chat.names} avatarVersion={avatarVersion} connected={chat.ready && !chat.error}/> : <><span className="matrix-terminal-dot"/><span>{tx("ui-matrix-chat-panel", "disconnected_session")}</span></>}</div><div className="matrix-terminal-activity"><MatrixGameActivities key={`${chat.userId}:${room}`} users={memberUsers} supporters={supporterUsers} messages={current?.messages ?? []}/></div><small>{preview ? tx("ui-matrix-chat-panel", "demo_no_real_sign_in_or_messages") : tx("ui-matrix-chat-panel", "community_chat")}</small></div>
      {!chat.userId ? <div className="matrix-terminal-welcome"><ConnectionStorageChoice service="matrix" disabled={busy||chat.pending} onChange={()=>void refreshMatrixStatus()}/><MatrixIcon/><h3>{tx("ui-matrix-chat-panel", "connect_to_the_community")}</h3><p>{tx("ui-matrix-chat-panel", "talk_about_games_mods_and_linux_in_a_terminal_style_chat_sign_in_with_your_matrix_accoun")}</p><button className="btn btn-primary" disabled={busy || chat.pending} onClick={() => void run("connect_matrix_chat").then(() => refreshMatrixStatus())}><MatrixIcon/>{chat.pending ? tx("ui-matrix-chat-panel", "waiting_for_sign_in") : tx("ui-matrix-chat-panel", "sign_in_with_matrix")}</button><p>{chat.pending ? tx("ui-matrix-chat-panel", "authorize_in_your_browser_and_return_here_sign_in_expires_in_three_minutes") : tx("ui-matrix-chat-panel", "your_password_is_not_saved_in_the_launcher")}</p></div> : <div className="matrix-terminal-layout">
        <aside className="matrix-terminal-rooms">
          {chat.ready && !communityJoined && <button className="matrix-terminal-join" disabled={busy} onClick={() => void join(MATRIX_COMMUNITY_ID)}>{tx("ui-matrix-chat-panel", "join_the_community")}</button>}
          {preview && <small className="matrix-rooms-demo">{tx("ui-matrix-chat-panel", "simulated_preview_rooms")}</small>}
          <div className="matrix-rooms-scroll">
          <MatrixChannelList rooms={rooms} room={room} onSelect={selectRoom} me={chat.userId} names={chat.names} joined={joinedRooms} ready={chat.ready} unread={unreadRooms}
            canCreate={canCreateChannels} locked={id => id === MATRIX_PREMIUM_ROOM_ID && !premiumReady} lockLabel={id => id === MATRIX_PREMIUM_ROOM_ID && !premiumReady ? " 🔒" : ""}
            onJoinRoom={async id => { if (!await run("matrix_chat_join", { room: id })) return false; await reloadMatrixRooms(); return true; }}
            onError={fail} refreshing={refreshingRooms} onRefresh={() => void refreshRooms()} permanent={id => id === MATRIX_COMMUNITY_ID || id === MATRIX_PREMIUM_ROOM_ID}
            onOpenChat={id => { setRoom(id); setCallChat(true); }} onEdit={(id, tab) => setChannelEdit({ room: id, tab })}
            canEdit={id => canCreateChannels || Boolean(chat.userId && chat.rooms[id]?.power && powerOf(chat.rooms[id]!.power, chat.userId) >= 50)}/>
          </div>
          <MatrixUserPanel onSettings={() => setSettingsOpen(true)}/>
        </aside>
        {inCall && !callChat ? <MatrixCallStage me={chat.userId} roomName={roomName} videoHidden={settingsOpen || Boolean(channelEdit)} onShowChat={() => setCallChat(true)}/> :
        <section className="matrix-terminal-room"><header><div className="matrix-room-heading"><strong title={roomName}># {roomName}</strong>{roomTopic && <small className="matrix-room-topic" title={roomTopic}>{roomTopic}</small>}</div>{inCall && <button type="button" className="btn btn-secondary" onClick={() => setCallChat(false)}>{tx("ui-matrix-chat-panel", "show_call")}</button>}{!premiumSelected && chat.ready && !current?.joined && <button className="btn btn-secondary" disabled={busy} onClick={() => void join(room)}>{tx("ui-matrix-chat-panel", "join_room")}</button>}</header>
          {premiumSelected && !premiumReady ? <div className="matrix-premium-gate" role="status">
            <h3>{roomName}</h3>
            <p>{!benefitsChecked || premium?.phase === "checking" ? tx("ui-matrix-chat-panel", "checking_discord_and_granting_matrix_access") : !exclusiveEffects ? tx("ui-matrix-chat-panel", "for_youtube_members_and_discord_server_boosters_sign_in_with_discord_in_preferences_conn") : (tx("ui-matrix-chat-panel", "confirm_your_connection_to_access_this_room"))}</p>
            {exclusiveEffects && premium?.phase !== "checking" && <button type="button" className="btn btn-secondary" onClick={() => setPremiumAttempt(value=>value+1)}>{tx("ui-matrix-chat-panel", "try_again")}</button>}
          </div> : <>
          <div ref={log} className="matrix-terminal-messages" role="log" aria-live="polite" aria-relevant="additions" onScroll={event => { const element = event.currentTarget; atBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80; }}>
            {current?.encrypted ? <p>{tx("ui-matrix-chat-panel", "this_room_uses_encryption_open_element_to_chat_securely")}</p> : <>
              {current?.prevBatch && messages.length > 0 && <button className="matrix-terminal-older" disabled={current.loadingOlder} onClick={() => void older()}>{current.loadingOlder ? tx("ui-matrix-chat-panel", "loading") : tx("ui-matrix-chat-panel", "load_older_messages")}</button>}
              {messages.length ? messages.map(message => {
                const name = displayName(message.sender, chat.names), color = userColor(message.sender);
                const replied = message.reply_to ? byId.get(message.reply_to) : undefined;
                return <article key={message.id} className={`matrix-terminal-message${chat.userId && mentionsMe(message, chat.userId, chat.names) ? " is-mention" : ""}${message.msgtype === "m.notice" ? " is-notice" : ""}`}>
                  <div className="matrix-terminal-avatar" style={{color,borderColor:color}} aria-hidden="true">{avatars[message.sender] ? <img src={avatars[message.sender]!} alt=""/> : name.slice(0,2).toUpperCase()}</div>
                  <div>
                    <header><strong style={{color}} title={message.sender}>{name}</strong>{supporterUsers.has(message.sender) && <MatrixSupporterBadge/>}<time>{new Date(message.ts).toLocaleTimeString(language,{hour:"2-digit",minute:"2-digit"})}</time>{message.edited && <small>{tx("ui-matrix-chat-panel", "edited")}</small>}
                      <span className="matrix-message-actions">
                        <button type="button" onClick={() => { setEditing(null); setReplyTo(message); input.current?.focus(); }}>{tx("ui-matrix-chat-panel", "reply")}</button>
                        <button type="button" aria-expanded={reacting === message.id} onClick={() => setReacting(reacting === message.id ? null : message.id)}>{tx("ui-matrix-chat-panel", "react")}</button>
                        <button type="button" className="matrix-message-more" aria-haspopup="menu" aria-expanded={messageMenu?.message.id === message.id} aria-label={tx("ui-matrix-chat-panel", "more_actions")} title={tx("ui-matrix-chat-panel", "more_actions")}
                          onClick={event => { const box = event.currentTarget.getBoundingClientRect(); setMessageMenu({ message, x: box.right, y: box.bottom + 4 }); }}>⋯</button>
                      </span>
                    </header>
                    {message.reply_to && <p className="matrix-message-reply">↳ {replied ? <><b>{displayName(replied.sender, chat.names)}</b>: {replied.body.slice(0, 120)}</> : tx("ui-matrix-chat-panel", "reply_to_an_earlier_message")}</p>}
                    <p>{message.msgtype === "m.emote" ? <>* <b style={{color}}>{name}</b> </> : null}{message.msgtype === "m.file" ? `[${tx("ui-matrix-chat-panel", "file")}] ` : null}{message.msgtype === "m.image" ? null : <MessageText text={message.body} onError={e => fail(e, tx("ui-matrix-chat-panel", "could_not_open_the_link"))}/>}</p>
                    {message.msgtype === "m.image" && message.url && <ChatImage mxc={message.url} animated={message.mimetype === "image/gif" || message.mimetype === "image/webp"}/>}
                    {message.msgtype === "m.video" && message.url && <ChatVideo mxc={message.url}/>}
                    {reacting === message.id && <div className="matrix-reaction-picker" role="group" aria-label={tx("ui-matrix-chat-panel", "choose_a_reaction")}>{REACTIONS.map(key => <button key={key} type="button" disabled={busy} onClick={() => void react(message, key)}>{key}</button>)}</div>}
                    {(current?.reactions[message.id]?.length ?? 0) > 0 && <div className="matrix-reactions">{groupReactions(current!.reactions[message.id]).map(([key, senders]) => {
                      const mine = Boolean(chat.userId && senders.includes(chat.userId));
                      return <button key={key} type="button" className={mine ? "is-mine" : undefined} aria-pressed={mine} disabled={busy} title={senders.map(user => displayName(user, chat.names)).join(", ")} onClick={() => void react(message, key)}>{key} {senders.length}</button>;
                    })}</div>}
                    {pending?.id === message.id && <form className={`matrix-message-report${pending.kind === "kick" || pending.kind === "ban" ? " is-moderation" : ""}`} onSubmit={event => { event.preventDefault(); void confirm(message, pending.kind); }}>
                      <strong>{actionText[pending.kind].title}{pending.kind === "kick" || pending.kind === "ban" ? `: ${name}` : ""}</strong>
                      <input value={reason} maxLength={pending.kind === "report" ? 1000 : 500} autoFocus onChange={event => setReason(event.target.value)} placeholder={tx("ui-matrix-chat-panel", "reason_optional")}/>
                      <button type="submit" className="btn btn-secondary" disabled={busy}>{actionText[pending.kind].button}</button>
                      <button type="button" className="btn btn-secondary" onClick={() => setPending(null)}>{tx("ui-matrix-chat-panel", "cancel")}</button>
                    </form>}
                  </div>
                </article>;
              }) : <p className="matrix-terminal-empty">{!chat.ready ? tx("ui-matrix-chat-panel", "connecting") : current?.joined ? tx("ui-matrix-chat-panel", "no_messages_yet_say_hi") : tx("ui-matrix-chat-panel", "join_the_room_to_see_the_messages")}</p>}
            </>}
          </div>
          <form className="matrix-terminal-compose" onSubmit={event=>{event.preventDefault();void send();}}>
            {typingNames.length > 0 && <p className="matrix-typing" aria-live="polite">{typingNames.length > 3 ? (tx("ui-matrix-chat-panel", "several_people_are_typing")) : `> ${typingNames.join(", ")} ${typingNames.length > 1 ? (tx("ui-matrix-chat-panel", "are_typing")) : (tx("ui-matrix-chat-panel", "is_typing"))}`}</p>}
            {editing ? <p className="matrix-compose-reply">✎ {tx("ui-matrix-chat-panel", "editing_your_message_esc_cancels")} <button type="button" onClick={() => { setEditing(null); setText(""); }} aria-label={tx("ui-matrix-chat-panel", "cancel_edit")}>✕</button></p>
              : replyTo ? <p className="matrix-compose-reply">↳ {tx("ui-matrix-chat-panel", "replying_to")} <b>{displayName(replyTo.sender, chat.names)}</b>: {replyTo.body.slice(0, 80)} <button type="button" onClick={() => setReplyTo(null)} aria-label={tx("ui-matrix-chat-panel", "cancel_reply")}>✕</button></p>
              : <label htmlFor="matrix-message">{tx("ui-matrix-chat-panel", "message_enter_sends_shift_enter_new_line_me_for_actions")}</label>}
            <div><input ref={fileInput} type="file" hidden accept="image/png,image/jpeg,image/gif,image/webp,.txt,.log" onChange={event=>{const file = event.target.files?.[0]; event.target.value = ""; if (file) void upload(file);}}/>
              <button type="button" className="btn btn-secondary matrix-attach-button" disabled={busy || current?.encrypted || !current?.joined} onClick={()=>fileInput.current?.click()} aria-label={tx("ui-matrix-chat-panel", "send_image_or_log")} title={tx("ui-matrix-chat-panel", "send_image_or_log")}><MenuIcon name="plus"/></button>
              <div className="matrix-message-field"><textarea ref={input} rows={1} id="matrix-message" value={text} maxLength={8000} disabled={current?.encrypted || !current?.joined} onChange={event=>{setText(event.target.value); typing(Boolean(event.target.value.trim()));}} onKeyDown={event => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void send(); } }} placeholder={tx("ui-matrix-chat-panel", "type_your_message")}/><MatrixEmojiPicker key={room} disabled={busy || Boolean(current?.encrypted) || !current?.joined} onSelect={insertEmoji}/></div>
              <button className="matrix-send-button" aria-label={tx("ui-matrix-chat-panel", "send_message_enter")} disabled={busy || current?.encrypted || !current?.joined || !text.trim()} type="submit">
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
      {messageMenu && chat.userId && (() => {
        const { message, x, y } = messageMenu, own = message.sender === chat.userId;
        const allowed = moderation(current?.power, chat.userId, message.sender);
        return <MatrixChannelMenu x={x} y={y} title={displayName(message.sender, chat.names)} onClose={() => setMessageMenu(null)} groups={[
          [
            { label: tx("ui-matrix-chat-panel", "profile"), action: () => setProfileOf({ user: message.sender, x, y }) },
            { label: tx("ui-matrix-chat-panel", "edit"), hidden: !own || (message.msgtype !== "m.text" && message.msgtype !== "m.emote"), action: () => { setReplyTo(null); setEditing(message); setText(message.body); input.current?.focus(); } },
          ],
          [
            { label: tx("ui-matrix-chat-panel", "report"), hidden: own, action: () => ask(message.id, "report") },
            { label: tx("ui-matrix-chat-panel", "block"), danger: true, hidden: own || busy, action: () => void setMatrixUserBlocked(message.sender, true).catch(error => fail(error, tx("ui-matrix-chat-panel", "could_not_block"))) },
          ],
          [
            { label: tx("ui-matrix-chat-panel", "remove_message"), danger: true, hidden: !allowed.redact, action: () => ask(message.id, "redact") },
            { label: tx("ui-matrix-chat-panel", "kick"), danger: true, hidden: !allowed.kick, action: () => ask(message.id, "kick") },
            { label: tx("ui-matrix-chat-panel", "ban"), danger: true, hidden: !allowed.ban, action: () => ask(message.id, "ban") },
          ],
        ]}/>;
      })()}
      {profileOf && <MatrixUserMenu target={profileOf} profile onClose={() => setProfileOf(null)}/>}
      {channelEdit && chat.userId && <MatrixChannelSettings key={channelEdit.room} room={channelEdit.room} names={chat.names} initialTab={channelEdit.tab} onClose={() => setChannelEdit(null)} onError={fail} onSaved={() => void reloadMatrixRooms()}/>}
      {settingsOpen && chat.userId && <MatrixSettings me={chat.userId} names={chat.names} onClose={() => setSettingsOpen(false)} onError={fail}
        onDisconnect={() => { setSettingsOpen(false); void disconnect(); }} onAvatarChanged={() => { setAvatarVersion(value => value + 1); setAvatars(previous => { const next = { ...previous }; delete next[chat.userId!]; return next; }); avatarLookups.current.delete(chat.userId!); }} avatarVersion={avatarVersion}/>}
      <footer className="matrix-terminal-footer"><h2 className="matrix-terminal-brand"><MatrixIcon/><span>Matrix</span><span className="matrix-terminal-tag">// Pelicanux</span></h2></footer>
      </div>
      </div>
    </ModalSurface>
  </div>,document.body);
}

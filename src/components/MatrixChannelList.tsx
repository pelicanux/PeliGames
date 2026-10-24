import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "./MenuIcon";
import { displayName, refreshMatrixRooms, type RoomInfo } from "../services/matrixChat";
import { joinMatrixVoice, leaveMatrixVoice, useMatrixVoice } from "../services/matrixVoice";
import { matrixSdkClient } from "../services/matrixSdk";
import { isVoiceRoom, useVoiceChannelMembers } from "../services/matrixVoiceChannels";
import { MatrixChannelMenu, channelLink, copyText } from "./MatrixChannelMenu";
import { MatrixUserMenu, type UserMenuTarget } from "./MatrixUserMenu";

/** Tabs of the channel editor that the right-click menu opens directly. */
export type ChannelEditTab = "overview" | "permissions" | "roles" | "invite";

const Speaker = () => <svg className="matrix-channel-speaker" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9H4Z"/><path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12"/></svg>;
/** Waits until the SDK sees the joined room (after joining it for a call). */
async function sdkRoomJoined(id: string) {
  const client = await matrixSdkClient();
  for (let waited = 0; waited < 15000; waited += 250) {
    if (client.getRoom(id)?.getMyMembership() === "join") return;
    await new Promise(resolve => setTimeout(resolve, 250));
  }
}

/** Discord-style channel list in the Matrix terminal: text and voice channels of the community. */
export function MatrixChannelList({ pt, rooms, room, onSelect, me, names, joined, ready, canCreate, locked, onJoinRoom, onError, refreshing, onRefresh, lockLabel, permanent, unread, onOpenChat, onEdit, canEdit }: {
  /** The text chat of a channel (for a voice channel, instead of its call view). */
  onOpenChat: (id: string) => void;
  /** Opens the channel editor; `canEdit` says whether this account may change anything there. */
  onEdit: (id: string, tab: ChannelEditTab) => void; canEdit: (id: string) => boolean;
  pt: boolean; rooms: RoomInfo[]; room: string; onSelect: (id: string) => void; me: string; names: Record<string, string>;
  /** Unread messages per room, from the server. */
  unread: Record<string, number>;
  /** Rooms the launcher never deletes (the community itself, the premium room). */
  permanent: (id: string) => boolean;
  joined: Set<string>; ready: boolean; canCreate: boolean; locked: (id: string) => boolean; lockLabel: (id: string) => string;
  onJoinRoom: (id: string) => Promise<boolean>; onError: (error: unknown, fallback: string) => void; refreshing: boolean; onRefresh: () => void;
}) {
  const voice = useMatrixVoice();
  const textRooms = rooms.filter(item => !isVoiceRoom(item)), voiceRooms = rooms.filter(isVoiceRoom);
  const members = useVoiceChannelMembers(voiceRooms.map(item => item.room_id).filter(id => joined.has(id)), ready);
  const [creating, setCreating] = useState<"text" | "voice" | null>(null), [name, setName] = useState(""), [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null), [removing, setRemoving] = useState(false);
  const switching = useRef(false);
  const [menu, setMenu] = useState<{ item: RoomInfo; x: number; y: number } | null>(null);
  const [userMenu, setUserMenu] = useState<UserMenuTarget | null>(null);
  const openMenu = (item: RoomInfo) => (event: React.MouseEvent) => { event.preventDefault(); event.stopPropagation(); setMenu({ item, x: event.clientX, y: event.clientY }); };
  const lastError = useRef("");
  useEffect(() => {
    // Call failures also reach the notification bell, once each.
    if (voice.error && voice.error !== lastError.current) onError(voice.error, voice.error);
    lastError.current = voice.error;
  }, [voice.error]);
  const callBusy = voice.phase === "joining" || voice.phase === "leaving";
  const enterVoice = async (id: string) => {
    onSelect(id);
    if (switching.current || callBusy || locked(id) || (voice.phase !== "idle" && voice.room === id)) return;
    switching.current = true;
    try {
      // As in Discord: clicking another voice channel moves the call there.
      if (voice.phase !== "idle") await leaveMatrixVoice();
      if (!joined.has(id)) {
        if (!await onJoinRoom(id)) return;
        if (!("__PELI_UI_PREVIEW__" in window)) await sdkRoomJoined(id);
      }
      await joinMatrixVoice(id);
    } finally { switching.current = false; }
  };
  const create = async () => {
    if (!creating || saving || !name.trim()) return;
    setSaving(true);
    try {
      const id = await invoke<string>("matrix_chat_create_channel", { name: name.trim(), kind: creating });
      await refreshMatrixRooms();
      setCreating(null); setName(""); onSelect(id);
    } catch (error) { onError(error, pt ? "Não foi possível criar o canal." : "Could not create the channel."); }
    finally { setSaving(false); }
  };
  const form = (kind: "text" | "voice") => creating === kind && <form className="matrix-channel-form" onSubmit={event => { event.preventDefault(); void create(); }}>
    <input value={name} maxLength={80} autoFocus disabled={saving} onChange={event => setName(event.target.value)} placeholder={kind === "voice" ? (pt ? "nome do canal de voz" : "voice channel name") : (pt ? "nome do canal de texto" : "text channel name")}/>
    <div><button type="submit" disabled={saving || !name.trim()}>{saving ? (pt ? "criando…" : "creating…") : (pt ? "criar" : "create")}</button><button type="button" disabled={saving} onClick={() => { setCreating(null); setName(""); }}>{pt ? "cancelar" : "cancel"}</button></div>
  </form>;
  const add = (kind: "text" | "voice") => canCreate && <button type="button" className="matrix-channel-add" aria-label={kind === "voice" ? (pt ? "Criar canal de voz" : "Create voice channel") : (pt ? "Criar canal de texto" : "Create text channel")} title={kind === "voice" ? (pt ? "Criar canal de voz" : "Create voice channel") : (pt ? "Criar canal de texto" : "Create text channel")} onClick={() => { setCreating(creating === kind ? null : kind); setName(""); }}><MenuIcon name="plus"/></button>;
  const remove = async (id: string) => {
    if (removing) return;
    setRemoving(true);
    try {
      if (voice.phase !== "idle" && voice.room === id) await leaveMatrixVoice();
      await invoke<number>("matrix_chat_delete_channel", { room: id });
      setDeleting(null);
      if (room === id) onSelect(rooms.find(item => item.room_id !== id)?.room_id ?? id);
      await refreshMatrixRooms();
    } catch (error) { onError(error, pt ? "Não foi possível apagar o canal." : "Could not delete the channel."); }
    finally { setRemoving(false); }
  };
  // As in Discord: the open channel shows no count; others show how many arrived.
  const badge = (id: string) => id !== room && (unread[id] ?? 0) > 0 ? <span className="matrix-channel-unread" aria-label={pt ? `${unread[id]} não lidas` : `${unread[id]} unread`}>{unread[id] > 99 ? "99+" : unread[id]}</span> : null;
  const confirmDelete = (item: RoomInfo) => deleting === item.room_id && <div className="matrix-channel-confirm" role="alertdialog" aria-label={pt ? "Apagar canal" : "Delete channel"}>
    <strong>{pt ? `> apagar ${isVoiceRoom(item) ? "" : "#"}${item.name || item.room_id}?` : `> delete ${isVoiceRoom(item) ? "" : "#"}${item.name || item.room_id}?`}</strong>
    <small>{pt ? "Todos saem do canal e ele some da comunidade. O histórico deixa de ser acessível pelo launcher. Não dá para desfazer." : "Everyone leaves the channel and it disappears from the community. Its history is no longer reachable from the launcher. This cannot be undone."}</small>
    <div><button type="button" className="matrix-channel-confirm-delete" disabled={removing} onClick={() => void remove(item.room_id)}>{removing ? (pt ? "apagando…" : "deleting…") : (pt ? "apagar" : "delete")}</button><button type="button" disabled={removing} onClick={() => setDeleting(null)}>{pt ? "cancelar" : "cancel"}</button></div>
  </div>;
  return <>
    <div className="matrix-rooms-heading"><h3>{pt ? "# CANAIS DE TEXTO" : "# TEXT CHANNELS"}</h3><span className="matrix-channel-heading-actions">{add("text")}<button type="button" className={refreshing ? "matrix-rooms-refresh is-refreshing" : "matrix-rooms-refresh"} disabled={refreshing} aria-busy={refreshing} onClick={onRefresh} aria-label={refreshing ? pt ? "Atualizando salas…" : "Refreshing rooms…" : pt ? "Atualizar salas" : "Refresh rooms"} title={refreshing ? pt ? "Atualizando salas…" : "Refreshing rooms…" : pt ? "Atualizar salas" : "Refresh rooms"}><MenuIcon name="repair"/></button></span></div>
    {form("text")}
    {textRooms.map(item => <div key={item.room_id} className="matrix-channel-row">
      <div className="matrix-channel-line"><button type="button" aria-current={room === item.room_id ? "page" : undefined} onContextMenu={openMenu(item)} onClick={() => onSelect(item.room_id)} className={badge(item.room_id) ? "has-unread" : undefined}># {item.name || item.room_id}{lockLabel(item.room_id)}{badge(item.room_id)}</button></div>
      {confirmDelete(item)}
    </div>)}
    {(voiceRooms.length > 0 || canCreate) && <div className="matrix-rooms-heading matrix-voice-heading"><h3>{pt ? "# CANAIS DE VOZ" : "# VOICE CHANNELS"}</h3><span className="matrix-channel-heading-actions">{add("voice")}</span></div>}
    {form("voice")}
    {voiceRooms.map(item => {
      const inCall = voice.phase !== "idle" && voice.room === item.room_id;
      const people = members[item.room_id] ?? [];
      return <div key={item.room_id} className="matrix-voice-channel matrix-channel-row">
        <div className="matrix-channel-line"><button type="button" className={inCall ? "is-in-call" : ""} aria-current={room === item.room_id ? "page" : undefined} onContextMenu={openMenu(item)} disabled={callBusy && !inCall} title={inCall ? (pt ? "Você está neste canal de voz" : "You are in this voice channel") : (pt ? "Entrar no canal de voz" : "Join voice channel")} onClick={() => void enterVoice(item.room_id)}><Speaker/><span className="matrix-channel-name">{item.name || item.room_id}{lockLabel(item.room_id)}</span></button></div>
        {confirmDelete(item)}
        {people.length > 0 && <ul className="matrix-voice-members" aria-label={pt ? `Na chamada de ${item.name}` : `In ${item.name}'s call`}>
          {people.map(person => <li key={person.userId} onContextMenu={event => { event.preventDefault(); event.stopPropagation(); setUserMenu({ user: person.userId, x: event.clientX, y: event.clientY }); }} className={[person.userId === me ? "is-me" : "", inCall && voice.speaking.includes(person.userId) ? "is-speaking" : ""].join(" ").trim() || undefined}><span className="matrix-voice-member-dot" aria-hidden="true"/>{displayName(person.userId, names)}{person.userId === me && inCall && (voice.muted || voice.deafened) ? <small>{voice.deafened ? (pt ? " (sem áudio)" : " (deafened)") : (pt ? " (mudo)" : " (muted)")}</small> : null}</li>)}
        </ul>}
      </div>;
    })}
    {userMenu && <MatrixUserMenu pt={pt} target={userMenu} onClose={() => setUserMenu(null)}/>}
    {menu && (() => {
      const { item } = menu, id = item.room_id, isVoice = isVoiceRoom(item);
      const inThisCall = voice.phase !== "idle" && voice.room === id;
      const copy = (text: string) => void copyText(text).catch(error => onError(error, pt ? "Não foi possível copiar." : "Could not copy."));
      return <MatrixChannelMenu x={menu.x} y={menu.y} title={`${isVoice ? "🔊 " : "# "}${item.name || id}`} onClose={() => setMenu(null)} groups={[
        [
          { label: inThisCall ? (pt ? "Sair da voz" : "Leave voice") : (pt ? "Entrar na voz" : "Join voice"), hidden: !isVoice || locked(id), action: () => void (inThisCall ? leaveMatrixVoice() : enterVoice(id)) },
          { label: pt ? "Abrir chat" : "Open chat", hidden: locked(id), action: () => onOpenChat(id) },
        ],
        [
          { label: pt ? "Convidar pessoas" : "Invite people", hidden: locked(id), action: () => onEdit(id, "invite") },
          { label: pt ? "Copiar link" : "Copy link", action: () => copy(channelLink(id)) },
          { label: pt ? "Copiar ID do canal" : "Copy channel ID", action: () => copy(id) },
        ],
        [
          { label: pt ? "Editar canal" : "Edit channel", hidden: !canEdit(id) || !joined.has(id), action: () => onEdit(id, "overview") },
          { label: pt ? "Apagar canal" : "Delete channel", danger: true, hidden: !canCreate || permanent(id), action: () => setDeleting(id) },
        ],
      ]}/>;
    })()}
  </>;
}

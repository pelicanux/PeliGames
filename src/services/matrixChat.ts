// App-wide state of the Matrix community chat: one long-polled /sync while
// signed in, so mentions reach the notification bell with the chat closed.
import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MATRIX_COMMUNITY_ID, MATRIX_PREMIUM_ROOM_ID } from "./matrixCommunity";

export interface ChatMessage { id: string; sender: string; ts: number; msgtype: string; body: string; url?: string | null; reply_to?: string | null; mentions?: string[]; edited?: boolean }
type Incoming = ChatMessage & { kind: "message" } | { kind: "edit"; id: string; target: string; body: string } | { kind: "redaction"; id: string; target: string };
/** Power levels of a room: who may remove messages, remove or ban people. */
export interface RoomPower { users: Record<string, number>; users_default: number; redact: number; kick: number; ban: number }
export interface ChatRoom { messages: ChatMessage[]; prevBatch: string | null; encrypted: boolean; joined: boolean; loadingOlder: boolean; power: RoomPower | null }
export interface RoomInfo { room_id: string; name?: string; room_type?: string; topic?: string }
export interface ChatState { userId: string | null; pending: boolean; error: string; roomList: RoomInfo[]; rooms: Record<string, ChatRoom>; names: Record<string, string>; ignored: string[]; ready: boolean }
interface SyncRoom { events: Incoming[]; prev_batch: string | null; limited: boolean; encrypted: boolean | null; names: Record<string, string>; power: RoomPower | null }
interface SyncResult { next_batch: string; user_id: string; join: Record<string, SyncRoom>; leave: string[]; ignored: string[] | null }

/** A message that mentions the signed-in user (bell): opens the chat on click. */
export interface MatrixMentionDetail { sender: string; room: string; message: string }
export const MATRIX_MENTION_EVENT = "peligamesMatrixMention";
export const OPEN_MATRIX_CHAT_EVENT = "peligamesOpenMatrixChat";
const MAX_MESSAGES = 300;

let state: ChatState = { userId: null, pending: false, error: "", roomList: [], rooms: {}, names: {}, ignored: [], ready: false };
const listeners = new Set<() => void>();
const set = (patch: Partial<ChatState>) => { state = { ...state, ...patch }; listeners.forEach(listener => listener()); };
let generation = 0, started = false, pendingTimer: ReturnType<typeof setTimeout> | undefined;
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const emptyRoom = (): ChatRoom => ({ messages: [], prevBatch: null, encrypted: false, joined: false, loadingOlder: false, power: null });
export const powerOf = (power: RoomPower | null | undefined, user: string) => power ? power.users[user] ?? power.users_default : 0;
/** What the signed-in user may do to a message of `sender` (the server checks again). */
export function moderation(power: RoomPower | null | undefined, me: string, sender: string) {
  const mine = powerOf(power, me), theirs = powerOf(power, sender), own = sender === me;
  return { redact: own || Boolean(power && mine >= power.redact), kick: !own && Boolean(power && mine >= power.kick && mine > theirs), ban: !own && Boolean(power && mine >= power.ban && mine > theirs) };
}

export const displayName = (user: string, names = state.names) => names[user] || user.split(":")[0].slice(1);
/** Messages addressed to `me`: Matrix mentions, or the user's name written in the text. */
export function mentionsMe(message: ChatMessage, me: string, names = state.names) {
  if (message.sender === me) return false;
  if (message.mentions?.includes(me)) return true;
  const text = message.body.toLowerCase();
  return [me.split(":")[0].slice(1), names[me]].filter((name): name is string => Boolean(name && name.length > 2)).some(name => text.includes(name.toLowerCase()));
}
/** Applies a batch (or older history) to a room: new messages, edits and removals. */
export function mergeEvents(messages: ChatMessage[], events: Incoming[], older = false) {
  const added = events.filter((event): event is ChatMessage & { kind: "message" } => event.kind === "message").map(({ kind: _kind, ...message }) => message);
  const known = new Set(messages.map(message => message.id));
  let next = older ? [...added.filter(message => !known.has(message.id)), ...messages] : [...messages, ...added.filter(message => !known.has(message.id))];
  for (const event of events) {
    if (event.kind === "edit") next = next.map(message => message.id === event.target ? { ...message, body: event.body, edited: true } : message);
    if (event.kind === "redaction") next = next.filter(message => message.id !== event.target);
  }
  return next.slice(-MAX_MESSAGES);
}

async function loadRooms(reportError = false) {
  const user = state.userId;
  try {
    const data = await invoke<{ rooms: RoomInfo[] }>("matrix_chat_rooms");
    if (state.userId !== user) return;
    const rooms = (data.rooms || []).filter(room => room.room_type !== "m.space");
    const discovered = rooms.length ? rooms : [{ room_id: MATRIX_COMMUNITY_ID, name: "PeliGames" }];
    set({ roomList: [...discovered.filter(room => room.room_id !== MATRIX_PREMIUM_ROOM_ID), { room_id: MATRIX_PREMIUM_ROOM_ID, name: "Suporte premium" }] });
  } catch (error) {
    if (reportError) throw error;
    if (state.userId === user) set({ roomList: [{ room_id: MATRIX_COMMUNITY_ID, name: "PeliGames" }] });
  }
}
function apply(data: SyncResult, initial: boolean) {
  const rooms = { ...state.rooms }, names = { ...state.names };
  for (const id of data.leave) rooms[id] = { ...(rooms[id] ?? emptyRoom()), joined: false };
  for (const [id, update] of Object.entries(data.join)) {
    Object.assign(names, update.names);
    const room = rooms[id] ?? emptyRoom();
    // A gap (limited timeline) restarts the room from this batch.
    const base = update.limited && !initial ? [] : room.messages;
    rooms[id] = { ...room, joined: true, encrypted: update.encrypted ?? room.encrypted, power: update.power ?? room.power, messages: mergeEvents(base, update.events),
      prevBatch: initial || update.limited || !room.prevBatch ? update.prev_batch : room.prevBatch };
    if (!initial && data.user_id) for (const event of update.events) {
      if (event.kind !== "message" || !mentionsMe(event, data.user_id, names) || state.ignored.includes(event.sender)) continue;
      const room = state.roomList.find(item => item.room_id === id)?.name || "PeliGames";
      window.dispatchEvent(new CustomEvent<MatrixMentionDetail>(MATRIX_MENTION_EVENT, { detail: { sender: displayName(event.sender, names), room, message: event.body.slice(0, 160) } }));
    }
  }
  set({ rooms, names, ignored: data.ignored ?? state.ignored, ready: true, error: "" });
}
async function syncLoop(run: number) {
  let since: string | null = null;
  while (run === generation) {
    try {
      const ids = [...new Set([MATRIX_COMMUNITY_ID, ...state.roomList.map(room => room.room_id)])];
      const data: SyncResult = await invoke<SyncResult>("matrix_chat_sync", { since, rooms: ids, timeout: since ? 30000 : 0 });
      if (run !== generation) return;
      apply(data, since === null);
      since = data.next_batch;
    } catch (error) {
      if (run !== generation) return;
      set({ error: String(error) });
      // An expired session is already cleared by the backend: show the sign-in again.
      if (/expirou|encerrada|Entre com Matrix/.test(String(error))) { await refreshMatrixStatus(); return; }
      await sleep(8000);
    }
  }
}
function stop() { generation++; set({ rooms: {}, names: {}, ignored: [], roomList: [], ready: false }); }
async function begin() {
  const run = ++generation;
  await loadRooms();
  if (run === generation) void syncLoop(run);
}
/** Reads the sign-in state; starts or stops the chat when it changes. */
export async function refreshMatrixStatus() {
  clearTimeout(pendingTimer);
  try {
    const data = await invoke<{ user_id: string | null; pending: boolean; error?: string | null }>("matrix_chat_status");
    const changed = data.user_id !== state.userId;
    set({ userId: data.user_id, pending: data.pending, error: data.error || (changed ? "" : state.error) });
    if (changed) { if (data.user_id) void begin(); else stop(); }
    // While the browser sign-in is open, check again until it finishes.
    if (data.pending) pendingTimer = setTimeout(() => void refreshMatrixStatus(), 2000);
  } catch (error) { set({ error: String(error) }); }
}
/** Starts once per app: an existing sign-in resumes the chat in the background. */
export function startMatrixChat() {
  if (started) return;
  started = true;
  void refreshMatrixStatus();
}
/** Reloads the room list and restarts the sync (after joining the community). */
export async function reloadMatrixRooms() {
  if (state.userId) await begin();
}
/** Refresh discovery without resetting the active conversation or its history. */
export async function refreshMatrixRooms() {
  const user = state.userId;
  if (!user) return;
  const previous = state.roomList.map(room => room.room_id).sort().join();
  await loadRooms(true);
  if (state.userId !== user) return;
  if (state.roomList.map(room => room.room_id).sort().join() !== previous) void syncLoop(++generation);
}
export async function loadOlderMessages(room: string) {
  const current = state.rooms[room];
  if (!current?.prevBatch || current.loadingOlder) return;
  set({ rooms: { ...state.rooms, [room]: { ...current, loadingOlder: true } } });
  try {
    const data = await invoke<{ events: Incoming[]; end: string | null; names: Record<string, string> }>("matrix_chat_history", { room, from: current.prevBatch });
    const latest = state.rooms[room] ?? current;
    set({ names: { ...data.names, ...state.names }, rooms: { ...state.rooms, [room]: { ...latest, loadingOlder: false, prevBatch: data.end && data.end !== current.prevBatch ? data.end : null, messages: mergeEvents(latest.messages, data.events, true) } } });
  } catch (error) {
    set({ rooms: { ...state.rooms, [room]: { ...(state.rooms[room] ?? current), loadingOlder: false } } });
    throw error;
  }
}
export function useMatrixChat() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, () => state);
}

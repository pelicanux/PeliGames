// App-wide state of the Matrix community chat: one long-polled /sync while
// signed in, so mentions reach the notification bell with the chat closed.
import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { startMatrixSdk, stopMatrixSdk, updateMatrixSdkRooms, retryMatrixSdk, matrixSdkClient } from "./matrixSdk";
import { fetchMatrixDisplayNames } from "./matrixDisplayNames";
import { matrixPresenceChoice, type MatrixMemberPresence } from "./matrixPresencePreferences";
import { MATRIX_COMMUNITY_ID } from "./matrixCommunity";

export interface ChatMessage { id: string; sender: string; ts: number; msgtype: string; body: string; url?: string | null; mimetype?: string | null; reply_to?: string | null; mentions?: string[]; edited?: boolean }
/** An emoji reaction (Matrix annotation) to a message. */
export interface Reaction { id: string; key: string; sender: string }
type Incoming = ChatMessage & { kind: "message" } | { kind: "edit"; id: string; target: string; body: string } | { kind: "redaction"; id: string; target: string } | Reaction & { kind: "reaction"; target: string };
/** Power levels of a room: who may remove messages, remove or ban people. */
export interface RoomPower { users: Record<string, number>; users_default: number; redact: number; kick: number; ban: number; space_child?: number; creators?: string[] }
export interface ChatRoom { creators: string[]; members: string[]; messages: ChatMessage[]; prevBatch: string | null; encrypted: boolean; joined: boolean; loadingOlder: boolean; power: RoomPower | null;
  /** Unread messages counted by the server; who is typing; reactions by message ID. */
  unread: number; typing: string[]; reactions: Record<string, Reaction[]> }
export interface RoomInfo { room_id: string; name?: string; room_type?: string; topic?: string }
export interface ChatState { userId: string | null; pending: boolean; error: string; roomList: RoomInfo[]; rooms: Record<string, ChatRoom>; names: Record<string, string>; ignored: string[]; ready: boolean; presence: Record<string, string>; memberPresence: Record<string, MatrixMemberPresence> }
interface SyncRoom { creators?: string[] | null; members?: Record<string, boolean>; events: Incoming[]; prev_batch: string | null; limited: boolean; encrypted: boolean | null; names: Record<string, string>; power: RoomPower | null; unread?: number | null; typing?: string[] | null }
interface SyncResult { next_batch: string; user_id: string; join: Record<string, SyncRoom>; leave: string[]; ignored: string[] | null; presence?: Record<string, string>; member_presence?: Record<string, MatrixMemberPresence> }

/** A message that mentions the signed-in user (bell): opens the chat on click. */
export interface MatrixMentionDetail { sender: string; room: string; message: string; silent?: boolean }
export const MATRIX_MENTION_EVENT = "peligamesMatrixMention";
export const OPEN_MATRIX_CHAT_EVENT = "peligamesOpenMatrixChat";
const MAX_MESSAGES = 300;

let state: ChatState = { userId: null, pending: false, error: "", roomList: [], rooms: {}, names: {}, ignored: [], ready: false, presence: {}, memberPresence: {} };
const listeners = new Set<() => void>();
const set = (patch: Partial<ChatState>) => { state = { ...state, ...patch }; listeners.forEach(listener => listener()); };
export function setMatrixPresence(user: string, status: string) {
  if (state.userId === user) set({ presence: { ...state.presence, [user]: status } });
}
let generation = 0, started = false, pendingTimer: ReturnType<typeof setTimeout> | undefined;
let statusRequest = 0;
let disconnectRequest: Promise<boolean> | null = null;
let profileRun = -1;
const profileAttempts = new Map<string, number>();
function resolveMissingNames() {
  if ("__PELI_UI_PREVIEW__" in window || !state.userId || profileRun === generation) return;
  const run = generation;
  profileRun = run;
  void (async () => {
    const sdk = await matrixSdkClient();
    while (run === generation) {
      const users = [...new Set(Object.values(state.rooms).flatMap(room => [...room.members, ...room.messages.map(message => message.sender)]))];
      const missing = users.filter(user => !state.names[user]?.trim() && Date.now() - (profileAttempts.get(user) ?? 0) > 60000);
      if (!missing.length) break;
      for (const user of missing) profileAttempts.set(user, Date.now());
      const names = await fetchMatrixDisplayNames(missing, user => sdk.getProfileInfo(user, "displayname"));
      if (run !== generation) return;
      // Room-specific names and updates received during the request take priority.
      const fallback = Object.fromEntries(Object.entries(names).filter(([user]) => !state.names[user]?.trim()));
      if (Object.keys(fallback).length) set({ names: { ...state.names, ...fallback } });
    }
  })().catch(() => {}).finally(() => { if (profileRun === run) profileRun = -1; });
}
const emptyRoom = (): ChatRoom => ({ creators: [], members: [], messages: [], prevBatch: null, encrypted: false, joined: false, loadingOlder: false, power: null, unread: 0, typing: [], reactions: {} });
const MAX_REACTED = 600;
/** Adds reactions and drops removed ones; a reaction may arrive before its message. */
export function mergeReactions(reactions: Record<string, Reaction[]>, events: Incoming[]) {
  let next = reactions;
  const copy = () => { if (next === reactions) next = { ...reactions }; };
  for (const event of events) {
    if (event.kind === "reaction") {
      const list = next[event.target] ?? [];
      if (list.some(item => item.id === event.id || (item.sender === event.sender && item.key === event.key))) continue;
      copy(); next[event.target] = [...list, { id: event.id, key: event.key, sender: event.sender }];
    } else if (event.kind === "redaction") {
      for (const [target, list] of Object.entries(next)) {
        if (!list.some(item => item.id === event.target)) continue;
        copy(); next[target] = list.filter(item => item.id !== event.target);
      }
    }
  }
  const targets = Object.keys(next);
  if (targets.length > MAX_REACTED) { copy(); for (const target of targets.slice(0, targets.length - MAX_REACTED)) delete next[target]; }
  return next;
}
// Since room version 12 the creators have unlimited power and are not in the users list.
export const powerOf = (power: RoomPower | null | undefined, user: string) => power?.creators?.includes(user) ? Infinity : power ? power.users[user] ?? power.users_default : 0;
/** The room's power levels with its v12 creators (who only appear in the create event). */
function withCreators(power: RoomPower | null, creators: string[]): RoomPower | null {
  if (power) return { ...power, creators };
  return creators.length ? { users: {}, users_default: 0, redact: 50, kick: 50, ban: 50, creators } : null;
}
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
    updateMatrixSdkRooms([MATRIX_COMMUNITY_ID, ...rooms.map(room => room.room_id)]);
    set({ roomList: rooms });
  } catch (error) {
    if (reportError) throw error;
    // Keep the last real room list on temporary discovery failures.
  }
}
/** Merge incremental membership changes; initial/rejoined rooms use a fresh snapshot. */
export function updateRoomMembers(previous: string[], changes: Record<string, boolean> = {}, replace = false): string[] {
  if (!replace && !Object.keys(changes).length) return previous;
  const members = new Set(replace ? [] : previous);
  for (const [user, joined] of Object.entries(changes)) { if (joined === true) members.add(user); else members.delete(user); }
  return [...members];
}
const memberRequests = new Map<string, { run: number; changes: Record<string, boolean>; promise: Promise<void> }>();
/** Sync can omit unchanged membership events. Hydrate the active room from the
 * authoritative roster, then replay any joins/leaves received during the request. */
export function refreshMatrixRoomMembers(roomId: string): Promise<void> {
  if ("__PELI_UI_PREVIEW__" in window || !state.userId || !state.rooms[roomId]?.joined) return Promise.resolve();
  const run = generation;
  const pending = memberRequests.get(roomId);
  if (pending?.run === run) return pending.promise;
  const request = { run, changes: {} as Record<string, boolean>, promise: Promise.resolve() };
  memberRequests.set(roomId, request);
  request.promise = (async () => {
    const sdk = await matrixSdkClient();
    if (run !== generation || !state.rooms[roomId]?.joined) return;
    const snapshot = await sdk.getJoinedRoomMembers(roomId);
    if (run !== generation || !state.rooms[roomId]?.joined) return;
    const room = state.rooms[roomId];
    const names = { ...state.names };
    for (const [user, profile] of Object.entries(snapshot.joined)) {
      if (!names[user]?.trim() && profile.display_name?.trim()) names[user] = [...profile.display_name.trim()].slice(0, 64).join("");
    }
    set({ names, rooms: { ...state.rooms, [roomId]: { ...room, members: updateRoomMembers(Object.keys(snapshot.joined), request.changes) } } });
    resolveMissingNames();
  })().finally(() => { if (memberRequests.get(roomId) === request) memberRequests.delete(roomId); });
  return request.promise;
}
function apply(data: SyncResult, initial: boolean) {
  const rooms = { ...state.rooms }, names = { ...state.names };
  for (const id of data.leave) rooms[id] = { ...(rooms[id] ?? emptyRoom()), joined: false, members: [], typing: [], unread: 0 };
  for (const [id, update] of Object.entries(data.join)) {
    const pending = memberRequests.get(id);
    if (pending?.run === generation) Object.assign(pending.changes, update.members);
    Object.assign(names, update.names);
    const room = rooms[id] ?? emptyRoom();
    // A gap (limited timeline) restarts the room from this batch.
    const base = update.limited && !initial ? [] : room.messages;
    const creators = update.creators ?? room.creators;
    rooms[id] = { ...room, creators, members: updateRoomMembers(room.members, update.members, initial || !room.joined), joined: true, encrypted: update.encrypted ?? room.encrypted, power: withCreators(update.power ?? room.power, creators), messages: mergeEvents(base, update.events),
      unread: update.unread ?? room.unread, typing: update.typing ?? room.typing, reactions: mergeReactions(room.reactions, update.events),
      prevBatch: initial || update.limited || !room.prevBatch ? update.prev_batch : room.prevBatch };
    if (!initial && data.user_id) for (const event of update.events) {
      if (event.kind !== "message" || !mentionsMe(event, data.user_id, names) || state.ignored.includes(event.sender)) continue;
      const room = state.roomList.find(item => item.room_id === id)?.name || "PeliGames";
      window.dispatchEvent(new CustomEvent<MatrixMentionDetail>(MATRIX_MENTION_EVENT, { detail: { sender: displayName(event.sender, names), room, message: event.body.slice(0, 160), silent: matrixPresenceChoice(data.user_id) === "dnd" } }));
    }
  }
  set({ rooms, names, ignored: data.ignored ?? state.ignored, ready: true, error: "", presence: { ...state.presence, ...data.presence }, memberPresence: { ...state.memberPresence, ...data.member_presence } });
  resolveMissingNames();
}
function syncLoop(run: number) {
  if ("__PELI_UI_PREVIEW__" in window) {
    // The preview stays a fixture; it never registers crypto keys or joins a call.
    void (async () => {
      let since: string | null = null;
      while (run === generation) {
        try {
          const data: SyncResult = await invoke<SyncResult>("matrix_chat_sync", { since, rooms: [MATRIX_COMMUNITY_ID, ...state.roomList.map(room => room.room_id)], timeout: since ? 30000 : 0 });
          if (run !== generation) return;
          apply(data, since === null); since = data.next_batch;
        } catch (error) {
          if (run !== generation) return;
          set({ error: String(error) });
          await new Promise(resolve => setTimeout(resolve, 8000));
        }
      }
    })();
    return;
  }
  void startMatrixSdk<SyncResult>(state.userId!, (data, initial) => {
    if (run === generation) apply(data, initial);
  }, error => {
    if (run !== generation) return;
    set({ error });
    if (/expirada|expirou|encerrada|Entre com Matrix|M_UNKNOWN_TOKEN/.test(error)) void refreshMatrixStatus();
  });
}
function stop() { generation++; memberRequests.clear(); profileAttempts.clear(); stopMatrixSdk(); window.dispatchEvent(new Event("peligamesMatrixSignedOut")); set({ rooms: {}, names: {}, ignored: [], roomList: [], ready: false, presence: {}, memberPresence: {} }); }
async function begin() {
  const run = ++generation;
  profileAttempts.clear();
  await loadRooms();
  if (run === generation) void syncLoop(run);
}
/** Reads the sign-in state; starts or stops the chat when it changes. */
export async function refreshMatrixStatus() {
  clearTimeout(pendingTimer);
  if (disconnectRequest) return;
  const request = ++statusRequest;
  try {
    const data = await invoke<{ user_id: string | null; pending: boolean; error?: string | null }>("matrix_chat_status");
    if (request !== statusRequest || disconnectRequest) return;
    const changed = data.user_id !== state.userId;
    set({ userId: data.user_id, pending: data.pending, error: data.error || (changed ? "" : state.error) });
    if (changed) { window.dispatchEvent(new Event("peligamesMatrixSignedOut")); if (data.user_id) void begin(); else stop(); }
    // While the browser sign-in is open, check again until it finishes.
    if (data.pending) pendingTimer = setTimeout(() => void refreshMatrixStatus(), 2000);
  } catch (error) { if (request === statusRequest && !disconnectRequest) set({ error: String(error) }); }
}
/** Clear shared chat state only after the backend has removed local credentials. */
export function disconnectMatrixChat(): Promise<boolean> {
  if (disconnectRequest) return disconnectRequest;
  ++statusRequest;
  clearTimeout(pendingTimer);
  disconnectRequest = invoke<boolean>("disconnect_matrix_chat").then(revoked => {
    ++statusRequest;
    set({ userId: null, pending: false, error: "" });
    stop();
    return revoked;
  }).finally(() => { disconnectRequest = null; });
  return disconnectRequest;
}
/** Starts once per app: an existing sign-in resumes the chat in the background. */
export function startMatrixChat() {
  if (started) return;
  started = true;
  void refreshMatrixStatus();
}
/** Reloads community discovery; the SDK follows joined rooms without restarting crypto. */
export async function reloadMatrixRooms() {
  if (state.userId) await loadRooms(true);
}
/** Refresh discovery without resetting the active conversation or its history. */
export async function refreshMatrixRooms() {
  const user = state.userId;
  if (!user) return;
  await loadRooms(true);
  if (state.userId !== user) return;
  retryMatrixSdk();
}
export async function loadOlderMessages(room: string) {
  const current = state.rooms[room];
  if (!current?.prevBatch || current.loadingOlder) return;
  set({ rooms: { ...state.rooms, [room]: { ...current, loadingOlder: true } } });
  try {
    const data = await invoke<{ events: Incoming[]; end: string | null; names: Record<string, string> }>("matrix_chat_history", { room, from: current.prevBatch });
    const latest = state.rooms[room] ?? current;
    set({ names: { ...data.names, ...state.names }, rooms: { ...state.rooms, [room]: { ...latest, loadingOlder: false, prevBatch: data.end && data.end !== current.prevBatch ? data.end : null, messages: mergeEvents(latest.messages, data.events, true), reactions: mergeReactions(latest.reactions, data.events) } } });
  } catch (error) {
    set({ rooms: { ...state.rooms, [room]: { ...(state.rooms[room] ?? current), loadingOlder: false } } });
    throw error;
  }
}
export function useMatrixChat() {
  return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, () => state);
}

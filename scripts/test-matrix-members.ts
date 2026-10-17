// Run separately: bun scripts/test-matrix-members.ts (no real Matrix requests).
import assert from "node:assert/strict";
import { mock } from "bun:test";

const room = "!community:test", me = "@me:test";
let signedIn: string | null = me;
let sync: (data: any, initial: boolean) => void;
let resolveRoster: (value: any) => void;
let rejectRoster: (error: Error) => void;
let requests = 0;
Object.assign(globalThis, { window: new EventTarget() });
mock.module("react", () => ({ useSyncExternalStore: (_subscribe: unknown, snapshot: () => unknown) => snapshot() }));
mock.module("@tauri-apps/api/core", () => ({ invoke: async (command: string) => {
  if (command === "matrix_chat_status") return { user_id: signedIn, pending: false };
  if (command === "matrix_chat_rooms") return { rooms: [{ room_id: room }] };
  throw new Error(`Unexpected native request: ${command}`);
} }));
const sdk = {
  getProfileInfo: async () => ({}),
  getJoinedRoomMembers: () => { requests++; return new Promise((resolve, reject) => { resolveRoster = resolve; rejectRoster = reject; }); },
};
mock.module("../src/services/matrixSdk", () => ({
  startMatrixSdk: (_user: string, callback: typeof sync) => { sync = callback; return Promise.resolve(sdk); },
  matrixSdkClient: async () => sdk, stopMatrixSdk() {}, updateMatrixSdkRooms() {}, retryMatrixSdk() {},
}));
const { refreshMatrixStatus, refreshMatrixRoomMembers, useMatrixChat, MATRIX_MENTION_EVENT } = await import("../src/services/matrixChat");
const { setMatrixPresenceChoice } = await import("../src/services/matrixPresencePreferences");
const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
const batch = (members: Record<string, boolean>, initial = false) => sync({
  user_id: me, next_batch: "s", leave: [], ignored: null,
  join: { [room]: { members, events: [], prev_batch: null, limited: false, encrypted: false, names: {}, power: null } },
}, initial);
await refreshMatrixStatus(); await flush();
batch({ "@a:test": true, "@b:test": true }, true);
const loading = refreshMatrixRoomMembers(room);
assert.equal(refreshMatrixRoomMembers(room), loading, "deduplicates concurrent roster requests");
await flush();
assert.equal(requests, 1);
batch({ "@b:test": false, "@new:test": true });
resolveRoster!({ joined: {
  [me]: { display_name: "Eu" }, "@a:test": { display_name: "Raquin" },
  "@b:test": { display_name: "Saiu" }, "@silent:test": { display_name: "Nunca escreveu" },
} });
await loading;
assert.deepEqual(new Set(useMatrixChat().rooms[room].members), new Set([me, "@a:test", "@silent:test", "@new:test"]));
assert.equal(useMatrixChat().names["@silent:test"], "Nunca escreveu");
assert.equal(useMatrixChat().names[me], "Eu");

const before = useMatrixChat().rooms[room].members;
const failed = refreshMatrixRoomMembers(room);
await flush(); rejectRoster!(new Error("offline"));
await assert.rejects(failed, /offline/);
assert.deepEqual(useMatrixChat().rooms[room].members, before, "temporary errors retain the existing roster");

const mentions: any[] = [];
window.addEventListener(MATRIX_MENTION_EVENT, event => mentions.push((event as CustomEvent).detail));
setMatrixPresenceChoice(me, "dnd");
sync({ user_id: me, next_batch: "mention", leave: [], ignored: null, join: { [room]: {
  members: {}, events: [{ kind: "message", id: "$mention", sender: "@a:test", ts: Date.now(), msgtype: "m.text", body: "Hello", mentions: [me] }],
  prev_batch: null, limited: false, encrypted: false, names: {}, power: null,
} } }, false);
assert.equal(mentions.length, 1);
assert.equal(mentions[0].silent, true, "DND retains the notification but flags it silent");

const stale = refreshMatrixRoomMembers(room);
await flush();
signedIn = null;
await refreshMatrixStatus();
resolveRoster!({ joined: { "@stale:test": { display_name: "Old account" } } });
await stale;
assert.equal(useMatrixChat().userId, null);
assert.deepEqual(useMatrixChat().rooms, {}, "late results cannot repopulate a signed-out session");
console.log("Passed: complete roster including self and silent members; names; concurrent deduplication; live joins/leaves; failure retention; sign-out race.");

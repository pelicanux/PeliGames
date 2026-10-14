import { test, expect } from "bun:test";
import { createClient, SyncState, type MatrixClient } from "matrix-js-sdk";
import { MatrixRTCSessionEvent, Status } from "matrix-js-sdk/lib/matrixrtc";
import { logger } from "matrix-js-sdk/lib/logger";

// Two real SDK crypto engines exchange Olm-encrypted to-device keys through an
// in-memory Matrix protocol fixture. No real accounts, microphones or network.
test("MatrixRTC exchanges encrypted media keys and rotates on leave", async () => {
  const users = ["@alice:example.org", "@bob:example.org"];
  const room = "!voice:example.org", service = "https://livekit-jwt.call.matrix.org";
  const devices = ["ALICE", "BOB"];
  const uploads: Record<string, any> = {};
  const inbox: Record<string, any[]> = Object.fromEntries(users.map(user => [user, []]));
  const state: any[] = [
    { type: "m.room.create", state_key: "", sender: users[0], content: { creator: users[0], room_version: "10" } },
    { type: "m.room.power_levels", state_key: "", sender: users[0], content: { users_default: 0, state_default: 0 } },
    ...users.map(user => ({ type: "m.room.member", state_key: user, sender: user, content: { membership: "join", displayname: user } })),
  ];
  let revision = 0, encryptedSends = 0;
  const clients: MatrixClient[] = [];
  const quiet = { trace() {}, debug() {}, info() {}, warn() {}, error() {}, getChild() { return quiet; } };
  logger.getChild = () => quiet;
  const wait = async (predicate: () => boolean, message: string) => {
    const deadline = Date.now() + 15000;
    while (!predicate()) { if (Date.now() > deadline) throw new Error(message); await Bun.sleep(20); }
  };
  try {
    for (let index = 0; index < 2; index++) {
      const user = users[index];
      const client = createClient({ baseUrl: "https://example.org", userId: user, deviceId: devices[index], accessToken: `test-${index}`, logger: quiet,
        fetchFn: async (input, init) => {
          const req = new Request(input, init), url = new URL(req.url), path = decodeURIComponent(url.pathname);
          const body = req.method === "GET" ? {} : JSON.parse(await req.text() || "{}");
          let result: any = {}, status = 200;
          if (path.includes("/room_keys/") || path.includes("/account_data/")) { status = 404; result = { errcode: "M_NOT_FOUND", error: "Not found" }; }
          else if (path.endsWith("/versions")) result = { versions: ["v1.11"], unstable_features: {} };
          else if (path.includes("/delayed_events") || path.includes("/unstable/")) { status = 404; result = { errcode: "M_UNRECOGNIZED", error: "Not supported by fixture" }; }
          else if (path.endsWith("/capabilities")) result = { capabilities: {} };
          else if (path.includes("/pushrules")) result = { global: { override: [], content: [], room: [], sender: [], underride: [] } };
          else if (path.endsWith("/filter")) result = { filter_id: "1" };
          else if (path.includes("/filter/")) result = {};
          else if (path.endsWith("/keys/upload")) {
            uploads[user] ??= { one_time_keys: {}, fallback_keys: {} };
            if (body.device_keys) uploads[user].device_keys = body.device_keys;
            Object.assign(uploads[user].one_time_keys, body.one_time_keys);
            Object.assign(uploads[user].fallback_keys, body.fallback_keys);
            result = { one_time_key_counts: { signed_curve25519: 50 } };
          } else if (path.endsWith("/keys/query")) {
            result = { device_keys: Object.fromEntries(Object.keys(body.device_keys).map(id => [id,
              uploads[id]?.device_keys ? { [uploads[id].device_keys.device_id]: uploads[id].device_keys } : {}])), failures: {} };
          } else if (path.endsWith("/keys/claim")) {
            result = { one_time_keys: {}, failures: {} };
            for (const [id, targets] of Object.entries(body.one_time_keys)) for (const device of Object.keys(targets as object)) {
              const stored = uploads[id], keyId = Object.keys(stored.one_time_keys)[0];
              result.one_time_keys[id] = { [device]: { [keyId]: stored.one_time_keys[keyId] } };
              delete stored.one_time_keys[keyId];
            }
          } else if (path.includes("/sendToDevice/")) {
            const type = path.split("/sendToDevice/")[1].split("/")[0];
            expect(type).toBe("m.room.encrypted"); encryptedSends++;
            for (const [target, messages] of Object.entries(body.messages)) for (const content of Object.values(messages as object)) {
              inbox[target].push({ type, sender: user, content });
            }
          } else if (path.includes("/state/org.matrix.msc3401.call.member/")) {
            const key = path.split("/state/org.matrix.msc3401.call.member/")[1];
            const existing = state.findIndex(event => event.type === "org.matrix.msc3401.call.member" && event.state_key === key);
            const event = { type: "org.matrix.msc3401.call.member", state_key: key, sender: user, content: body, origin_server_ts: Date.now(), event_id: `$event${++revision}` };
            if (existing < 0) state.push(event); else state[existing] = event;
            result = { event_id: event.event_id };
          } else if (path.endsWith("/sync")) {
            if (url.searchParams.has("since")) await Bun.sleep(30);
            result = { next_batch: `s${++revision}`, rooms: { join: { [room]: { state: { events: state }, timeline: { events: [], prev_batch: "old", limited: false }, ephemeral: { events: [] }, account_data: { events: [] } } } },
              to_device: { events: inbox[user].splice(0) }, device_lists: { changed: url.searchParams.has("since") ? [] : users, left: [] }, device_one_time_keys_count: { signed_curve25519: 50 }, account_data: { events: [] }, presence: { events: [] } };
          } else if (path.endsWith("/joined_members")) result = { joined: Object.fromEntries(users.map(id => [id, { display_name: id }])) };
          else if (path.endsWith("/members")) result = { chunk: state.filter(event => event.type === "m.room.member") };
          else if (path.endsWith("/state")) result = state;
          else if (path.endsWith("/whoami")) result = { user_id: user, device_id: devices[index] };
          return new Response(JSON.stringify(result), { status, headers: { "Content-Type": "application/json" } });
        },
      });
      clients.push(client);
      await client.initRustCrypto({ useIndexedDB: false });
      await client.startClient({ initialSyncLimit: 10, lazyLoadMembers: false });
    }
    await wait(() => clients.every(client => [SyncState.Prepared, SyncState.Syncing].includes(client.getSyncState()!)), "SDK sync failed");
    const sessions = clients.map(client => client.matrixRTC.getRoomSession(client.getRoom(room)!));
    await Promise.all(sessions.map(session => session.initialMembershipCalculated));
    const keys = [new Map<string, string>(), new Map<string, string>()];
    sessions.forEach((session, index) => {
      session.on(MatrixRTCSessionEvent.EncryptionKeyChanged, (key, keyIndex, _member, identity) => keys[index].set(`${identity}:${keyIndex}`, Buffer.from(key).toString("hex")));
      session.join({ userId: users[index], deviceId: devices[index], memberId: `${users[index]}:${devices[index]}` }, [{ type: "livekit", livekit_service_url: service }], { manageMediaKeys: true, callIntent: "audio", keyRotationGracePeriodMs: 100, useKeyDelay: 100 });
    });
    await wait(() => sessions.every(session => session.membershipStatus === Status.Connected && session.memberships.length === 2), "RTC membership did not converge");
    await wait(() => keys.every(ring => [...ring.keys()].some(id => id.startsWith(`${users[0]}:${devices[0]}:`)) && [...ring.keys()].some(id => id.startsWith(`${users[1]}:${devices[1]}:`))), "Encrypted to-device media keys were not delivered");
    for (const [id, value] of keys[0]) if (keys[1].has(id)) expect(keys[1].get(id)).toBe(value);
    expect(encryptedSends).toBeGreaterThan(0);
    inbox[users[0]].push({ type: "io.element.call.encryption_keys", sender: users[1], content: {
      room_id: room, keys: { index: 222, key: Buffer.alloc(16, 99).toString("base64") },
      member: { id: `${users[1]}:${devices[1]}`, claimed_device_id: devices[1] }, sent_ts: Date.now(),
    } });
    await Bun.sleep(100);
    expect(keys[0].has(`${users[1]}:${devices[1]}:222`)).toBe(false);
    const before = keys[0].size;
    await sessions[1].leave(1000);
    await wait(() => sessions[0].memberships.length === 1 && keys[0].size > before, "Key did not rotate after participant left");
    await sessions[0].leave(1000);
  }
  finally { clients.forEach(client => client.stopClient()); }
}, 45000);

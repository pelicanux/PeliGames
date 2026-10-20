// One SDK owns /sync, including to-device messages, for this Matrix device.
// The native bridge inserts credentials; SDK requests carry a placeholder only.
import { invoke } from "@tauri-apps/api/core";
import type { MatrixClient } from "matrix-js-sdk";
import { matrixPresenceChoice, matrixWirePresence } from "./matrixPresencePreferences";

let client: MatrixClient | null = null;
let ready: Promise<MatrixClient> | null = null;
let epoch = 0;
let failed = false;
let restart: (() => Promise<MatrixClient>) | null = null;
let communityRooms: string[] = [];
let voiceRoom: string | null = null;
export function updateMatrixSdkRooms(rooms: string[]) { communityRooms = [...new Set(rooms)].slice(0, 60); }
export function setMatrixSdkVoiceRoom(room: string | null) { voiceRoom = room; }
const quiet = { trace() {}, debug() {}, info() {}, warn() {}, error() {}, getChild() { return quiet; } };

export function matrixSdkClient() {
  if (client?.isInitialSyncComplete()) return Promise.resolve(client);
  if (failed && restart) return restart();
  if (!ready) return Promise.reject(new Error("Aguarde a conexão do Matrix."));
  return ready;
}
export function stopMatrixSdk() {
  epoch++;
  client?.stopClient();
  client = null;
  ready = null; failed = false; restart = null;
}
export function retryMatrixSdk() {
  if (client?.isInitialSyncComplete()) client.retryImmediately();
  else if (failed) void restart?.();
}
export function startMatrixSdk<T>(user: string, onSync: (data: T, initial: boolean) => void, onError: (error: string) => void) {
  stopMatrixSdk();
  const run = epoch;
  restart = () => startMatrixSdk(user, onSync, onError);
  let initial = true;
  let lastRooms = "";
  ready = (async () => {
    const [{ createClient, ClientEvent, SyncState }, { logger }, identity] = await Promise.all([
      import("matrix-js-sdk"),
      import("matrix-js-sdk/lib/logger"),
      invoke<{ user_id: string; device_id: string; base_url: string; storage_key: number[]; temporary?:boolean }>("matrix_rtc_identity"),
    ]);
    if (run !== epoch || identity.user_id !== user) throw new Error("A conta Matrix mudou.");
    logger.getChild = () => quiet;
    const sdk = createClient({
      baseUrl: identity.base_url, userId: user, deviceId: identity.device_id,
      accessToken: "native-keyring", logger: quiet,
      fetchFn: async (input, init) => {
        const request = new Request(input, init);
        if (run !== epoch || request.signal.aborted) throw new DOMException("Aborted", "AbortError");
        const url = new URL(request.url);
        let roomsSignature = "";
        if (url.pathname.endsWith("/sync")) {
          url.searchParams.set("set_presence", matrixWirePresence(matrixPresenceChoice(user)));
          roomsSignature = JSON.stringify([...new Set([...communityRooms, ...(voiceRoom ? [voiceRoom] : [])])].sort());
          if (roomsSignature !== lastRooms) url.searchParams.set("full_state", "true");
          url.searchParams.set("filter", JSON.stringify({ room: {
            rooms: [...new Set([...communityRooms, ...(voiceRoom ? [voiceRoom] : [])])],
            timeline: { limit: 50 }, state: { lazy_load_members: false }, ephemeral: { types: ["m.typing"] },
          } }));
        }
        const body = request.method === "GET" ? null : await request.text();
        const result = await invoke<{ status: number; body: unknown; chat: T | null }>("matrix_rtc_request", {
          url: url.href, method: request.method, body: body || null, userId: user,
        });
        if (run !== epoch || request.signal.aborted) throw new DOMException("Aborted", "AbortError");
        if (result.chat) { lastRooms = roomsSignature; onSync(result.chat, initial); initial = false; }
        return new Response(JSON.stringify(result.body), { status: result.status, headers: { "Content-Type": "application/json" } });
      },
    });
    client = sdk;
    await sdk.initRustCrypto({ useIndexedDB: !identity.temporary, storageKey: new Uint8Array(identity.storage_key), cryptoDatabasePrefix: `peligames-matrix-${encodeURIComponent(user)}-${encodeURIComponent(identity.device_id)}` });
    if (run !== epoch) { sdk.stopClient(); throw new Error("Conexão Matrix cancelada."); }
    sdk.on(ClientEvent.Sync, (state, _previous, data) => {
      if (run === epoch && state === SyncState.Error) onError(data?.error?.message || "Sem conexão com o Matrix.");
    });
    await sdk.startClient({ initialSyncLimit: 50, lazyLoadMembers: false, pollTimeout: 30000 });
    await new Promise<void>((resolve, reject) => {
      if (sdk.getSyncState() === SyncState.Prepared || sdk.getSyncState() === SyncState.Syncing) { resolve(); return; }
      const timer = setTimeout(() => { sdk.off(ClientEvent.Sync, update); reject(new Error("O Matrix demorou para sincronizar. Tente novamente.")); }, 60000);
      const update = (state: string) => {
        if (state === SyncState.Prepared || state === SyncState.Syncing) { clearTimeout(timer); sdk.off(ClientEvent.Sync, update); resolve(); }
      };
      sdk.on(ClientEvent.Sync, update);
    });
    return sdk;
  })();
  void ready.catch(error => { if (run === epoch) { failed = true; onError(String(error)); } });
  return ready;
}

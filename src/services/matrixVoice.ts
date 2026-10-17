import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { matrixSdkClient, setMatrixSdkVoiceRoom } from "./matrixSdk";
import type { MatrixRTCSession } from "matrix-js-sdk/lib/matrixrtc";

export type NoiseMode = "off" | "standard" | "rnnoise";
type MediaKey = { identity: string; index: number; key: number[] };
/** Audio preferences, kept between calls and launcher sessions (as in Discord). */
export type VoiceSettings = {
  muted: boolean; deafened: boolean;
  /** Reception volume and microphone volume, in percent (0 to 200). */
  volume: number; micVolume: number;
  noise: NoiseMode; agc: boolean;
  /** PipeWire device names; null follows the system default. */
  input: string | null; output: string | null;
};
type VoiceState = VoiceSettings & {
  phase: "idle" | "joining" | "connected" | "reconnecting" | "leaving";
  /** Native screen share of this account: picking a source in the portal, or live. */
  screen: "off" | "choosing" | "live";
  /** Call identities ("user:device") sharing a screen, other than this launcher. */
  shares: string[];
  /** The share shown on the stage or the mini player: an identity, "local" for the own preview. */
  watching: string | null;
  room: string | null; error: string;
  participants: { id: string; name: string }[];
  /** Matrix users speaking now in the call (including this account). */
  speaking: string[];
};
const SETTINGS_KEY = "peligames.matrixVoice";
const DEFAULTS: VoiceSettings = { muted: true, deafened: false, volume: 100, micVolume: 100, noise: "rnnoise", agc: true, input: null, output: null };
function loadSettings(): VoiceSettings {
  try {
    const saved = JSON.parse(localStorage.getItem(SETTINGS_KEY) || "{}") as Partial<VoiceSettings>;
    // The earlier noise-only preference.
    const legacy = localStorage.getItem("peligames.matrixNoiseMode");
    const percent = (value: unknown, fallback: number) => typeof value === "number" && value >= 0 && value <= 200 ? Math.round(value) : fallback;
    const device = (value: unknown) => typeof value === "string" && value.length <= 256 ? value : null;
    const noise = saved.noise ?? legacy;
    return {
      muted: typeof saved.muted === "boolean" ? saved.muted : DEFAULTS.muted,
      deafened: saved.deafened === true,
      volume: percent(saved.volume, DEFAULTS.volume), micVolume: percent(saved.micVolume, DEFAULTS.micVolume),
      noise: noise === "off" || noise === "standard" || noise === "rnnoise" ? noise : DEFAULTS.noise,
      agc: typeof saved.agc === "boolean" ? saved.agc : DEFAULTS.agc,
      input: device(saved.input), output: device(saved.output),
    };
  } catch { return { ...DEFAULTS }; }
}
const settingsOf = (value: VoiceState): VoiceSettings => ({ muted: value.muted, deafened: value.deafened, volume: value.volume, micVolume: value.micVolume, noise: value.noise, agc: value.agc, input: value.input, output: value.output });
let state: VoiceState = { ...loadSettings(), screen: "off", shares: [], watching: null, phase: "idle", room: null, error: "", participants: [], speaking: [] };
const subscribers = new Set<() => void>();
const set = (patch: Partial<VoiceState>) => { state = { ...state, ...patch }; subscribers.forEach(fn => fn()); };
let session: MatrixRTCSession | null = null;
let cleanup: (() => void) | null = null;
let generation = 0;
let nativeStarted = false;
let currentCallId: string | null = null;
let pendingNative: Promise<void> | null = null;
let leaving: Promise<void> | null = null;
let nativeListeners: Promise<UnlistenFn[]> | null = null;
const SERVICE = "https://livekit-jwt.call.matrix.org";

function attachNative() {
  nativeListeners ??= Promise.all([
    listen<{ state: "connected" | "reconnecting"; call_id?: string }>("matrix-call-status", ({ payload }) => {
      if (payload.call_id === currentCallId && state.phase !== "idle" && state.phase !== "leaving") set({ phase: payload.state });
    }),
    listen<{ call_id?: string; identities: string[] }>("matrix-call-speaking", ({ payload }) => {
      if (payload.call_id !== currentCallId) return;
      // Identities are "user:device"; Matrix user IDs contain one ":" themselves.
      set({ speaking: [...new Set(payload.identities.map(identity => identity.split(":").slice(0, 2).join(":")))] });
    }),
    listen<{ state: "choosing" | "live" | "off"; call_id?: string; error?: string | null }>("matrix-screen-status", ({ payload }) => {
      if (payload.call_id !== currentCallId) return;
      set({ screen: payload.state, ...(payload.error ? { error: payload.error } : {}) });
      // As in Discord: going live shows the own stream; stopping takes it away.
      if (payload.state === "live" && !state.watching) void watchMatrixScreen("local");
      if (payload.state === "off" && state.watching === "local") void watchMatrixScreen(null);
    }),
    listen<{ call_id?: string; identities: string[] }>("matrix-screen-shares", ({ payload }) => {
      if (payload.call_id !== currentCallId) return;
      set({ shares: payload.identities });
      if (state.watching && state.watching !== "local" && !payload.identities.includes(state.watching)) set({ watching: null });
    }),
    listen<{ call_id?: string; error?: string } | string | null>("matrix-call-ended", ({ payload }) => {
      if (payload && typeof payload === "object" && payload.call_id === currentCallId && state.phase !== "idle" && state.phase !== "leaving") void leaveMatrixVoice(payload.error || "");
    }),
  ]);
  return nativeListeners;
}
export function leaveMatrixVoice(error = ""): Promise<void> {
  if (leaving) return leaving;
  ++generation;
  set({ phase: "leaving", error });
  nativeStarted = false;
  const previous = session;
  session = null;
  cleanup?.(); cleanup = null;
  leaving = (async () => {
    await invoke("matrix_call_stop").catch(() => {});
    // Keep new joins blocked until an in-flight native start has acknowledged cancellation.
    await pendingNative?.catch(() => {});
    await invoke("matrix_call_stop").catch(() => {});
    if (previous) await previous.leave(4000).catch(() => {});
    currentCallId = null;
    setMatrixSdkVoiceRoom(null);
    set({ phase: "idle", room: null, participants: [], speaking: [], screen: "off", shares: [], watching: null, error });
  })().finally(() => { leaving = null; });
  return leaving;
}
export async function joinMatrixVoice(roomId: string, mode: NoiseMode = state.noise) {
  if (state.phase !== "idle") return;
  const run = ++generation;
  const callId = crypto.randomUUID();
  currentCallId = callId;
  setMatrixSdkVoiceRoom(roomId);
  set({ phase: "joining", room: roomId, error: "", speaking: [] });
  const current = () => run === generation;
  const keys = new Map<string, MediaKey>();
  try {
    await attachNative();
    const [client, { MatrixRTCSessionEvent, MembershipManagerEvent, Status }] = await Promise.all([
      matrixSdkClient(), import("matrix-js-sdk/lib/matrixrtc"),
    ]);
    if (!current()) return;
    const room = client.getRoom(roomId);
    if (!room || room.getMyMembership() !== "join") throw new Error("Entre na sala Matrix antes de conectar à voz.");
    const userId = client.getUserId()!, deviceId = client.getDeviceId()!;
    // Element Call encrypts the media only in encrypted rooms; elsewhere every
    // client sends and expects plain audio, so the native side must match.
    const encrypted = room.hasEncryptionStateEvent();
    const rtc = client.matrixRTC.getRoomSession(room);
    session = rtc;
    await rtc.initialMembershipCalculated;
    if (!current()) return;
    const unsupported = rtc.memberships.some(member => {
      const focus = member.getTransport(rtc.memberships[0]);
      return focus && (focus.type !== "livekit" || !("livekit_service_url" in focus) || focus.livekit_service_url !== SERVICE);
    });
    if (unsupported) throw new Error("Esta chamada usa outro serviço de voz. A conexão entre serviços ainda não está disponível.");
    const updateParticipants = () => {
      if (!current()) return;
      set({ participants: rtc.memberships.map(member => ({
        id: `${member.userId}:${member.deviceId}`,
        name: room.getMember(member.userId)?.name || member.userId,
      })) });
      if (room.getMyMembership() !== "join") void leaveMatrixVoice("Você não participa mais desta sala.");
    };
    const onKey = (key: Uint8Array, index: number, _membership: unknown, identity: string) => {
      if (!current() || !encrypted) return;
      const value = { identity, index, key: Array.from(key) };
      keys.set(`${identity}:${index}`, value);
      if (nativeStarted) void invoke("matrix_room_call_key", { callId, key: value }).catch(() => {
        if (current()) void leaveMatrixVoice("Não foi possível atualizar a criptografia da chamada.");
      });
    };
    const onFailure = () => { if (current()) void leaveMatrixVoice("O Matrix não conseguiu manter sua participação na chamada. Tente entrar novamente."); };
    rtc.on(MatrixRTCSessionEvent.EncryptionKeyChanged, onKey);
    rtc.on(MatrixRTCSessionEvent.MembershipsChanged, updateParticipants);
    rtc.on(MatrixRTCSessionEvent.MembershipManagerError, onFailure);
    const onProbablyLeft = (left: boolean) => { if (left) onFailure(); };
    rtc.on(MembershipManagerEvent.ProbablyLeft, onProbablyLeft);
    cleanup = () => {
      rtc.off(MatrixRTCSessionEvent.EncryptionKeyChanged, onKey);
      rtc.off(MatrixRTCSessionEvent.MembershipsChanged, updateParticipants);
      rtc.off(MatrixRTCSessionEvent.MembershipManagerError, onFailure);
      rtc.off(MembershipManagerEvent.ProbablyLeft, onProbablyLeft);
      keys.clear();
    };
    rtc.join({ userId, deviceId, memberId: `${userId}:${deviceId}` }, [{ type: "livekit", livekit_service_url: SERVICE }], {
      manageMediaKeys: encrypted, callIntent: "audio", membershipEventExpiryMs: 90000,
      membershipEventExpiryHeadroomMs: 30000, keyRotationGracePeriodMs: 1000,
    });
    // Voice authorization runs while Matrix confirms the membership; the native start reuses it.
    void invoke("matrix_room_call_prepare", { room: roomId }).catch(() => {});
    if (encrypted) rtc.reemitEncryptionKeys();
    const deadline = Date.now() + 45000;
    while (current() && (rtc.membershipStatus !== Status.Connected || (encrypted && ![...keys.values()].some(key => key.identity === `${userId}:${deviceId}`)))) {
      if (Date.now() > deadline) throw new Error("O Matrix não confirmou a entrada e as chaves de voz. Confira as permissões da sala e tente novamente.");
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    if (!current()) return;
    await invoke("matrix_audio_route", { input: state.input, output: state.output }).catch(() => {});
    pendingNative = invoke<void>("matrix_room_call_start", { room: roomId, mode, callId, keys: encrypted ? [...keys.values()] : [], encrypted });
    try { await pendingNative; } finally { pendingNative = null; }
    if (!current()) return;
    nativeStarted = true;
    // The call starts muted; then the saved microphone, volumes and filters apply.
    await pushControls();
    if (encrypted) for (const key of keys.values()) await invoke("matrix_room_call_key", { callId, key });
    updateParticipants();
  } catch (error) {
    if (current()) await leaveMatrixVoice(String(error).replace(/^Error: /, ""));
  }
}
async function pushControls() {
  if (!nativeStarted) return;
  // Deafened: nothing is heard and nothing is sent, as in Discord.
  try { await invoke("matrix_call_controls", { muted: state.muted || state.deafened, volume: state.deafened ? 0 : state.volume, noise: state.noise, micVolume: state.micVolume, agc: state.agc }); }
  catch { await leaveMatrixVoice("Não foi possível aplicar o controle de áudio. A chamada foi encerrada."); }
}
/** Changes audio preferences: saved, and applied at once to a running call. */
export async function setMatrixVoiceControls(patch: Partial<VoiceSettings>) {
  const devicesChanged = ("input" in patch && patch.input !== state.input) || ("output" in patch && patch.output !== state.output);
  set(patch);
  try { localStorage.setItem(SETTINGS_KEY, JSON.stringify(settingsOf(state))); } catch { /* Kept for this session only. */ }
  if (devicesChanged) await invoke("matrix_audio_route", { input: state.input, output: state.output });
  await pushControls();
}
/** Starts or stops sharing the screen in the current call (the system picks the source). */
/** Sound sent with a screen share: none, the whole PC except the call, or one application. */
export type ShareAudio = { kind: "none" } | { kind: "all" } | { kind: "app"; key: string };
/** Starts sharing the screen (the system then asks which screen or window), or stops with `null`. */
export async function shareMatrixScreen(audio: ShareAudio | null) {
  if (!nativeStarted || state.phase !== "connected") return;
  set({ screen: audio ? "choosing" : "off", error: "" });
  try { await invoke("matrix_screen_share", { on: Boolean(audio), audio }); }
  catch (error) { set({ screen: "off", error: String(error).replace(/^Error: /, "") }); }
}
/** Shows a share on the stage (or the mini player), the own preview with "local", or nothing. */
export async function watchMatrixScreen(identity: string | null) {
  if (!nativeStarted) { set({ watching: null }); return; }
  set({ watching: identity });
  try { await invoke("matrix_screen_watch", { identity }); }
  catch { set({ watching: null }); }
}
/** The call this launcher is in (or joining), if any; read outside React. */
export function matrixVoiceRoom() { return state.phase === "idle" ? null : state.room; }
export function useMatrixVoice() {
  return useSyncExternalStore(fn => { subscribers.add(fn); return () => subscribers.delete(fn); }, () => state);
}
window.addEventListener("peligamesMatrixSignedOut", () => {
  if (state.phase !== "idle") void leaveMatrixVoice();
});
window.addEventListener("pagehide", () => { if (state.phase !== "idle") void leaveMatrixVoice(); });

// Voice channels of the community: rooms of Element's call type. Who is in
// each one comes from the MatrixRTC memberships the SDK already syncs, so the
// list shows people in a channel without joining its call.
import { useEffect, useState } from "react";
import type { MatrixClient } from "matrix-js-sdk";
import { matrixSdkClient } from "./matrixSdk";
import { useMatrixVoice, matrixVoiceRoom } from "./matrixVoice";
import type { RoomInfo } from "./matrixChat";

export const VOICE_ROOM_TYPES = ["org.matrix.msc3417.call", "io.element.video"];
export const isVoiceRoom = (room: Pick<RoomInfo, "room_type">) => VOICE_ROOM_TYPES.includes(room.room_type ?? "");
export type VoiceMember = { userId: string; deviceId: string };

const CALL_MEMBER = "org.matrix.msc3401.call.member";
const clearing = new Set<string>();
/**
 * A call membership of this very device while the launcher is not in that
 * call: left behind when the launcher closed during a call (the same Matrix
 * session opens again). It is ended, so nobody sees a ghost in the channel.
 * Other devices of the account (a phone in the call) are left alone.
 */
function endStaleOwnMembership(client: MatrixClient, roomId: string) {
  if (matrixVoiceRoom() === roomId) return;
  const userId = client.getUserId(), deviceId = client.getDeviceId();
  const state = client.getRoom(roomId)?.getLiveTimeline().getState("f" as never);
  for (const event of state?.getStateEvents(CALL_MEMBER) ?? []) {
    const content = event.getContent();
    const key = `${roomId}|${event.getStateKey()}`;
    if (event.getSender() !== userId || content.device_id !== deviceId || !Object.keys(content).length || clearing.has(key)) continue;
    clearing.add(key);
    void client.sendStateEvent(roomId, CALL_MEMBER as never, {} as never, event.getStateKey())
      .catch(() => {}).finally(() => clearing.delete(key));
  }
}

/** People in the call of each joined voice room, kept up to date. */
export function useVoiceChannelMembers(roomIds: string[], ready: boolean) {
  const [members, setMembers] = useState<Record<string, VoiceMember[]>>({});
  const key = roomIds.join("|");
  const voice=useMatrixVoice();
  useEffect(() => {
    if (!ready || !roomIds.length || "__PELI_UI_PREVIEW__" in window) { setMembers({}); return; }
    let active = true;
    const cleanups: (() => void)[] = [];
    void (async () => {
      const [client, { MatrixRTCSessionEvent, MatrixRTCSessionManagerEvents }] = await Promise.all([matrixSdkClient(), import("matrix-js-sdk/lib/matrixrtc")]);
      if (!active) return;
      const read = (id: string): VoiceMember[] => {
        const room = client.getRoom(id);
        if (!room) return [];
        const seen = new Set<string>();
        // One entry per person, even with several devices in the call.
        return client.matrixRTC.getRoomSession(room).memberships
          .filter(member => member.userId && !seen.has(member.userId) && seen.add(member.userId))
          .map(member => ({ userId: member.userId!, deviceId: member.deviceId ?? "" }));
      };
      const update = () => {
        if (!active) return;
        for (const id of roomIds) endStaleOwnMembership(client, id);
        setMembers(Object.fromEntries(roomIds.map(id => [id, read(id)])));
      };
      for (const id of roomIds) {
        const room = client.getRoom(id);
        if (!room) continue;
        const session = client.matrixRTC.getRoomSession(room);
        session.on(MatrixRTCSessionEvent.MembershipsChanged, update);
        cleanups.push(() => session.off(MatrixRTCSessionEvent.MembershipsChanged, update));
      }
      client.matrixRTC.on(MatrixRTCSessionManagerEvents.SessionStarted, update);
      client.matrixRTC.on(MatrixRTCSessionManagerEvents.SessionEnded, update);
      cleanups.push(() => { client.matrixRTC.off(MatrixRTCSessionManagerEvents.SessionStarted, update); client.matrixRTC.off(MatrixRTCSessionManagerEvents.SessionEnded, update); });
      update();
    })().catch(() => {});
    return () => { active = false; cleanups.forEach(cleanup => cleanup()); };
  }, [key, ready]);
  if("__PELI_UI_PREVIEW__" in window)return Object.fromEntries(roomIds.map(id=>[id,voice.room===id&&voice.phase!=="idle"?voice.participants.map(person=>({userId:person.id,deviceId:"preview"})):[]]));
  return members;
}

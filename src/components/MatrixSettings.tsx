import {useI18n} from "../i18n/I18nContext";
// Settings behind the gear: profile (name, picture), voice and audio
// (devices, volumes, filters) and account, in the Matrix terminal look.
import {MatrixShortcutSettings} from "./MatrixShortcutSettings";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SelectionMenu } from "./SelectionMenu";
import { MatrixOwnAvatar, noiseOptions } from "./MatrixUserPanel";
import { displayName, setMatrixUserBlocked, useMatrixChat } from "../services/matrixChat";
import { forgetMatrixAvatar } from "../services/matrixMedia";
import { setMatrixVoiceControls, useMatrixVoice, type NoiseMode } from "../services/matrixVoice";
import { setMatrixScreenFit, useMatrixScreenFit, type ScreenFit } from "../services/matrixScreenView";

type Tab = "profile" | "audio" | "screen" | "shortcuts" | "blocked" | "account";
type Device = { id: string; name: string };
const SYSTEM = "__system__";

export function MatrixSettings({ me, names, onClose, onError, onDisconnect, onAvatarChanged, avatarVersion }: {
  me: string; names: Record<string, string>; onClose: () => void;
  onError: (error: unknown, fallback: string) => void; onDisconnect: () => void;
  onAvatarChanged: () => void; avatarVersion: number;
}) {
  const {t: tx, language} = useI18n();

  const voice = useMatrixVoice();
  const screenFit = useMatrixScreenFit();
  const [tab, setTab] = useState<Tab>("profile");
  const chat = useMatrixChat();
  const [unblocking, setUnblocking] = useState<string | null>(null);
  const unblock = async (user: string) => {
    setUnblocking(user);
    try { await setMatrixUserBlocked(user, false); }
    catch (error) { onError(error, tx("ui-matrix-settings", "could_not_unblock")); }
    finally { setUnblocking(null); }
  };
  const [name, setName] = useState(displayName(me, names));
  const [saving, setSaving] = useState<"" | "name" | "avatar">("");
  const [notice, setNotice] = useState("");
  const [devices, setDevices] = useState<{ inputs: Device[]; outputs: Device[] } | null>(null);
  const [meter, setMeter] = useState(0);
  const file = useRef<HTMLInputElement>(null);
  const frame = useRef<HTMLDivElement>(null);
  useEffect(() => { frame.current?.querySelector<HTMLButtonElement>("button")?.focus(); }, []);
  useEffect(() => {
    if (tab !== "audio") return;
    let active = true;
    void invoke<{ inputs: Device[]; outputs: Device[] }>("matrix_audio_devices").then(value => { if (active) setDevices(value); }).catch(() => { if (active) setDevices({ inputs: [], outputs: [] }); });
    return () => { active = false; };
  }, [tab]);
  // The microphone level, as sent (after volume and filters), while in a call.
  useEffect(() => {
    if (tab !== "audio" || voice.phase !== "connected") { setMeter(0); return; }
    const stop = listen<{ output: number }>("matrix-voice-meter", event => setMeter(Math.min(1, event.payload.output * 6)));
    return () => { void stop.then(unlisten => unlisten()); };
  }, [tab, voice.phase]);
  const saveName = async () => {
    if (saving || !name.trim()) return;
    setSaving("name"); setNotice("");
    try { await invoke("matrix_profile_set_name", { name: name.trim() }); setNotice(tx("ui-matrix-settings", "name_updated_in_every_room")); }
    catch (error) { onError(error, tx("ui-matrix-settings", "could_not_change_the_name")); }
    finally { setSaving(""); }
  };
  const saveAvatar = async (picked: File) => {
    if (saving) return;
    if (picked.size > 2 * 1024 * 1024) { onError(null, tx("ui-matrix-settings", "the_image_must_be_at_most_2_mb")); return; }
    setSaving("avatar"); setNotice("");
    try {
      await invoke("matrix_profile_set_avatar", new Uint8Array(await picked.arrayBuffer()), { headers: { "x-matrix-mime": picked.type } });
      forgetMatrixAvatar(me); onAvatarChanged();
      setNotice(tx("ui-matrix-settings", "profile_picture_updated"));
    } catch (error) { onError(error, tx("ui-matrix-settings", "could_not_change_the_picture")); }
    finally { setSaving(""); }
  };
  const deviceOptions = (list: Device[] | undefined, chosen: string | null) => {
    const options = [{ value: SYSTEM, label: tx("ui-matrix-settings", "system_default") }, ...(list ?? []).map(device => ({ value: device.id, label: device.name }))];
    // A saved device that is unplugged now stays visible, marked as missing.
    if (chosen && list && !list.some(device => device.id === chosen)) options.push({ value: chosen, label: tx("ui-matrix-settings", "disconnected", {"chosen": chosen}) });
    return options;
  };
  const slider = (label: string, value: number, change: (value: number) => void) => <label className="matrix-settings-slider">
    <span>{label}<b>{value}%</b></span>
    <input type="range" min="0" max="200" step="5" value={value} onChange={event => void change(Number(event.target.value))}/>
  </label>;
  const tabs: [Tab, string][] = [["profile", tx("ui-matrix-settings", "profile")], ["audio", tx("ui-matrix-settings", "voice_audio")], ["screen", tx("ui-matrix-settings", "screen")], ["shortcuts", tx("ui-matrix-settings", "shortcuts")], ["blocked", tx("ui-matrix-settings", "blocked")], ["account", tx("ui-matrix-settings", "account")]];
  return <div className="matrix-settings-overlay" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}
    onKeyDown={event => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); onClose(); } }}>
    <div ref={frame} className="matrix-settings" role="dialog" aria-modal="true" aria-label={tx("ui-matrix-settings", "settings")}>
      <header><strong>{tx("ui-matrix-settings", "settings_2")}</strong><button type="button" className="matrix-settings-close" onClick={onClose} aria-label={tx("ui-matrix-settings", "close_settings")}>[x]</button></header>
      <nav role="tablist">{tabs.map(([id, label]) => <button key={id} type="button" role="tab" aria-selected={tab === id} onClick={() => { setTab(id); setNotice(""); }}>{label}</button>)}</nav>
      <div className="matrix-settings-body" role="tabpanel">
        {tab === "shortcuts" && <MatrixShortcutSettings/>}
        {tab === "blocked" && <div className="matrix-settings-field">
          <span>{tx("ui-matrix-settings", "blocked_users")}</span>
          <small>{tx("ui-matrix-settings", "messages_from_people_you_block_disappear_from_every_room_the_list_is_kept_in_your_matrix")}</small>
          {chat.ignored.length ? <ul className="matrix-settings-blocked">{chat.ignored.map(user => <li key={user}>
            <span><b>{displayName(user, chat.names)}</b><code>{user}</code></span>
            <button type="button" disabled={unblocking === user} onClick={() => void unblock(user)}>{unblocking === user ? (tx("ui-matrix-settings", "unblocking")) : (tx("ui-matrix-settings", "unblock"))}</button>
          </li>)}</ul> : <p className="matrix-settings-hint">{tx("ui-matrix-settings", "nobody_blocked")}</p>}
        </div>}
        {tab === "profile" && <>
          <div className="matrix-settings-profile">
            <MatrixOwnAvatar user={me} name={displayName(me, names)} version={avatarVersion}/>
            <div>
              <input ref={file} type="file" hidden accept="image/png,image/jpeg,image/webp,image/gif" onChange={event => { const picked = event.target.files?.[0]; event.target.value = ""; if (picked) void saveAvatar(picked); }}/>
              <button type="button" disabled={Boolean(saving)} onClick={() => file.current?.click()}>{saving === "avatar" ? (tx("ui-matrix-settings", "uploading")) : (tx("ui-matrix-settings", "change_picture"))}</button>
              <small>{tx("ui-matrix-settings", "png_jpeg_webp_or_gif_up_to_2_mb")}</small>
            </div>
          </div>
          <form className="matrix-settings-field" onSubmit={event => { event.preventDefault(); void saveName(); }}>
            <label htmlFor="matrix-settings-name">{tx("ui-matrix-settings", "display_name")}</label>
            <div><input id="matrix-settings-name" value={name} maxLength={64} disabled={Boolean(saving)} onChange={event => setName(event.target.value)}/>
              <button type="submit" disabled={Boolean(saving) || !name.trim() || name.trim() === displayName(me, names)}>{saving === "name" ? (tx("ui-matrix-settings", "saving")) : (tx("ui-matrix-settings", "save"))}</button></div>
            <small>{tx("ui-matrix-settings", "applies_to_every_matrix_room_element_included")}</small>
          </form>
        </>}
        {tab === "audio" && <>
          <div className="matrix-settings-field"><span>{tx("ui-matrix-settings", "microphone_input")}</span>
            <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={tx("ui-matrix-settings", "microphone")} value={voice.input ?? SYSTEM} options={deviceOptions(devices?.inputs, voice.input)}
              onChange={value => void setMatrixVoiceControls({ input: value === SYSTEM ? null : value }).catch(error => onError(error, tx("ui-matrix-settings", "could_not_change_the_microphone")))}/></div>
          <div className="matrix-settings-field"><span>{tx("ui-matrix-settings", "audio_output")}</span>
            <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={tx("ui-matrix-settings", "audio_output_2")} value={voice.output ?? SYSTEM} options={deviceOptions(devices?.outputs, voice.output)}
              onChange={value => void setMatrixVoiceControls({ output: value === SYSTEM ? null : value }).catch(error => onError(error, tx("ui-matrix-settings", "could_not_change_the_audio_output")))}/></div>
          {devices && !devices.inputs.length && <small className="matrix-settings-hint">{tx("ui-matrix-settings", "device_list_unavailable_pipewire_pulseaudio_did_not_answer_calls_use_the_system_defaults")}</small>}
          {slider(tx("ui-matrix-settings", "microphone_volume"), voice.micVolume, value => setMatrixVoiceControls({ micVolume: value }))}
          <div className="matrix-settings-meter" aria-label={tx("ui-matrix-settings", "microphone_level")}><span style={{ width: `${Math.round(meter * 100)}%` }}/></div>
          <small className="matrix-settings-hint">{voice.phase === "connected" ? (tx("ui-matrix-settings", "the_meter_shows_what_others_receive_still_while_muted")) : (tx("ui-matrix-settings", "the_meter_works_during_a_call"))}</small>
          {slider(tx("ui-matrix-settings", "reception_volume"), voice.volume, value => setMatrixVoiceControls({ volume: value }))}
          <div className="matrix-settings-field"><span>{tx("ui-matrix-settings", "noise_suppression")}</span>
            <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={tx("ui-matrix-settings", "noise_suppression_2")} value={voice.noise} options={noiseOptions(language)} onChange={value => void setMatrixVoiceControls({ noise: value as NoiseMode })}/></div>
          <label className="matrix-settings-check"><input type="checkbox" checked={voice.agc} onChange={event => void setMatrixVoiceControls({ agc: event.target.checked })}/>
            <span>{tx("ui-matrix-settings", "automatic_microphone_gain")}<small>{tx("ui-matrix-settings", "evens_out_your_voice_when_you_speak_softly_or_move_away_from_the_microphone")}</small></span></label>
        </>}
        {tab === "screen" && <div className="matrix-settings-field">
          <span>{tx("ui-matrix-settings", "full_screen_picture")}</span>
          <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={tx("ui-matrix-settings", "stream_framing")} value={screenFit}
            options={[{ value: "fill", label: tx("ui-matrix-settings", "fill_the_frame") }, { value: "contain", label: tx("ui-matrix-settings", "fit_the_entire_picture") }]}
            onChange={value => setMatrixScreenFit(value as ScreenFit)}/>
          <small>{screenFit === "fill" ? (tx("ui-matrix-settings", "fills_the_curved_screen_without_stretching_edges_may_be_cropped")) : (tx("ui-matrix-settings", "shows_the_entire_picture_inside_the_frame_with_margins_when_needed"))}</small>
        </div>}
        {tab === "account" && <>
          <div className="matrix-settings-field"><span>{tx("ui-matrix-settings", "matrix_account")}</span><code>{me}</code></div>
          <small className="matrix-settings-hint">{tx("ui-matrix-settings", "disconnecting_ends_this_launcher_session_on_matrix_your_rooms_and_messages_stay_in_the_a")}</small>
          <button type="button" className="matrix-settings-danger" onClick={onDisconnect}>{tx("ui-matrix-settings", "disconnect")}</button>
        </>}
        {notice && <p className="matrix-settings-notice" role="status">{notice}</p>}
      </div>
    </div>
  </div>;
}

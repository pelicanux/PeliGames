// Settings behind the gear: profile (name, picture), voice and audio
// (devices, volumes, filters) and account, in the Matrix terminal look.
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SelectionMenu } from "./SelectionMenu";
import { MatrixOwnAvatar, noiseOptions } from "./MatrixUserPanel";
import { displayName } from "../services/matrixChat";
import { forgetMatrixAvatar } from "../services/matrixMedia";
import { setMatrixVoiceControls, useMatrixVoice, type NoiseMode } from "../services/matrixVoice";

type Tab = "profile" | "audio" | "account";
type Device = { id: string; name: string };
const SYSTEM = "__system__";

export function MatrixSettings({ pt, me, names, onClose, onError, onDisconnect, onAvatarChanged, avatarVersion }: {
  pt: boolean; me: string; names: Record<string, string>; onClose: () => void;
  onError: (error: unknown, fallback: string) => void; onDisconnect: () => void;
  onAvatarChanged: () => void; avatarVersion: number;
}) {
  const voice = useMatrixVoice();
  const [tab, setTab] = useState<Tab>("profile");
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
    try { await invoke("matrix_profile_set_name", { name: name.trim() }); setNotice(pt ? "> nome atualizado em todas as salas" : "> name updated in every room"); }
    catch (error) { onError(error, pt ? "Não foi possível trocar o nome." : "Could not change the name."); }
    finally { setSaving(""); }
  };
  const saveAvatar = async (picked: File) => {
    if (saving) return;
    if (picked.size > 2 * 1024 * 1024) { onError(null, pt ? "A imagem deve ter até 2 MB." : "The image must be at most 2 MB."); return; }
    setSaving("avatar"); setNotice("");
    try {
      await invoke("matrix_profile_set_avatar", new Uint8Array(await picked.arrayBuffer()), { headers: { "x-matrix-mime": picked.type } });
      forgetMatrixAvatar(me); onAvatarChanged();
      setNotice(pt ? "> foto de perfil atualizada" : "> profile picture updated");
    } catch (error) { onError(error, pt ? "Não foi possível trocar a foto." : "Could not change the picture."); }
    finally { setSaving(""); }
  };
  const deviceOptions = (list: Device[] | undefined, chosen: string | null) => {
    const options = [{ value: SYSTEM, label: pt ? "Padrão do sistema" : "System default" }, ...(list ?? []).map(device => ({ value: device.id, label: device.name }))];
    // A saved device that is unplugged now stays visible, marked as missing.
    if (chosen && list && !list.some(device => device.id === chosen)) options.push({ value: chosen, label: pt ? `${chosen} (desconectado)` : `${chosen} (disconnected)` });
    return options;
  };
  const slider = (label: string, value: number, change: (value: number) => void) => <label className="matrix-settings-slider">
    <span>{label}<b>{value}%</b></span>
    <input type="range" min="0" max="200" step="5" value={value} onChange={event => void change(Number(event.target.value))}/>
  </label>;
  const tabs: [Tab, string][] = [["profile", pt ? "# perfil" : "# profile"], ["audio", pt ? "# voz e áudio" : "# voice & audio"], ["account", pt ? "# conta" : "# account"]];
  return <div className="matrix-settings-overlay" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}
    onKeyDown={event => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); onClose(); } }}>
    <div ref={frame} className="matrix-settings" role="dialog" aria-modal="true" aria-label={pt ? "Configurações" : "Settings"}>
      <header><strong>{pt ? "> configurações" : "> settings"}</strong><button type="button" className="matrix-settings-close" onClick={onClose} aria-label={pt ? "Fechar configurações" : "Close settings"}>[x]</button></header>
      <nav role="tablist">{tabs.map(([id, label]) => <button key={id} type="button" role="tab" aria-selected={tab === id} onClick={() => { setTab(id); setNotice(""); }}>{label}</button>)}</nav>
      <div className="matrix-settings-body" role="tabpanel">
        {tab === "profile" && <>
          <div className="matrix-settings-profile">
            <MatrixOwnAvatar user={me} name={displayName(me, names)} version={avatarVersion}/>
            <div>
              <input ref={file} type="file" hidden accept="image/png,image/jpeg,image/webp,image/gif" onChange={event => { const picked = event.target.files?.[0]; event.target.value = ""; if (picked) void saveAvatar(picked); }}/>
              <button type="button" disabled={Boolean(saving)} onClick={() => file.current?.click()}>{saving === "avatar" ? (pt ? "[enviando…]" : "[uploading…]") : (pt ? "[trocar foto]" : "[change picture]")}</button>
              <small>{pt ? "PNG, JPEG, WebP ou GIF, até 2 MB." : "PNG, JPEG, WebP or GIF, up to 2 MB."}</small>
            </div>
          </div>
          <form className="matrix-settings-field" onSubmit={event => { event.preventDefault(); void saveName(); }}>
            <label htmlFor="matrix-settings-name">{pt ? "> nome de exibição" : "> display name"}</label>
            <div><input id="matrix-settings-name" value={name} maxLength={64} disabled={Boolean(saving)} onChange={event => setName(event.target.value)}/>
              <button type="submit" disabled={Boolean(saving) || !name.trim() || name.trim() === displayName(me, names)}>{saving === "name" ? (pt ? "[salvando…]" : "[saving…]") : (pt ? "[salvar]" : "[save]")}</button></div>
            <small>{pt ? "Vale para todas as salas Matrix, inclusive no Element." : "Applies to every Matrix room, Element included."}</small>
          </form>
        </>}
        {tab === "audio" && <>
          <div className="matrix-settings-field"><span>{pt ? "> microfone (entrada)" : "> microphone (input)"}</span>
            <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={pt ? "Microfone" : "Microphone"} value={voice.input ?? SYSTEM} options={deviceOptions(devices?.inputs, voice.input)}
              onChange={value => void setMatrixVoiceControls({ input: value === SYSTEM ? null : value }).catch(error => onError(error, pt ? "Não foi possível trocar o microfone." : "Could not change the microphone."))}/></div>
          <div className="matrix-settings-field"><span>{pt ? "> saída de áudio" : "> audio output"}</span>
            <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={pt ? "Saída de áudio" : "Audio output"} value={voice.output ?? SYSTEM} options={deviceOptions(devices?.outputs, voice.output)}
              onChange={value => void setMatrixVoiceControls({ output: value === SYSTEM ? null : value }).catch(error => onError(error, pt ? "Não foi possível trocar a saída de áudio." : "Could not change the audio output."))}/></div>
          {devices && !devices.inputs.length && <small className="matrix-settings-hint">{pt ? "Lista de dispositivos indisponível (o PipeWire/PulseAudio não respondeu). A chamada usa os padrões do sistema." : "Device list unavailable (PipeWire/PulseAudio did not answer). Calls use the system defaults."}</small>}
          {slider(pt ? "> volume do microfone" : "> microphone volume", voice.micVolume, value => setMatrixVoiceControls({ micVolume: value }))}
          <div className="matrix-settings-meter" aria-label={pt ? "Nível do microfone" : "Microphone level"}><span style={{ width: `${Math.round(meter * 100)}%` }}/></div>
          <small className="matrix-settings-hint">{voice.phase === "connected" ? (pt ? "O medidor mostra o que os outros recebem (fica parado com o microfone mudo)." : "The meter shows what others receive (still while muted).") : (pt ? "O medidor funciona durante uma chamada." : "The meter works during a call.")}</small>
          {slider(pt ? "> volume da recepção" : "> reception volume", voice.volume, value => setMatrixVoiceControls({ volume: value }))}
          <div className="matrix-settings-field"><span>{pt ? "> redução de ruído" : "> noise suppression"}</span>
            <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={pt ? "Redução de ruído" : "Noise suppression"} value={voice.noise} options={noiseOptions(pt)} onChange={value => void setMatrixVoiceControls({ noise: value as NoiseMode })}/></div>
          <label className="matrix-settings-check"><input type="checkbox" checked={voice.agc} onChange={event => void setMatrixVoiceControls({ agc: event.target.checked })}/>
            <span>{pt ? "Ganho automático do microfone" : "Automatic microphone gain"}<small>{pt ? "Iguala o volume da sua voz quando você fala baixo ou se afasta do microfone." : "Evens out your voice when you speak softly or move away from the microphone."}</small></span></label>
        </>}
        {tab === "account" && <>
          <div className="matrix-settings-field"><span>{pt ? "> conta Matrix" : "> Matrix account"}</span><code>{me}</code></div>
          <small className="matrix-settings-hint">{pt ? "Desconectar encerra esta sessão do launcher no Matrix. Suas salas e mensagens continuam na conta." : "Disconnecting ends this launcher session on Matrix. Your rooms and messages stay in the account."}</small>
          <button type="button" className="matrix-settings-danger" onClick={onDisconnect}>{pt ? "[desconectar]" : "[disconnect]"}</button>
        </>}
        {notice && <p className="matrix-settings-notice" role="status">{notice}</p>}
      </div>
    </div>
  </div>;
}

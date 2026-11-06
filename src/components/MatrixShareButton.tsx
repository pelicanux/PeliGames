import {useI18n} from "../i18n/I18nContext";
// "Share screen" with the choice of sound, as in Discord. The system's screen
// picker (the Wayland portal) has no audio option, so the sound is chosen here
// first: none, the whole PC except the call, or one application.
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { shareMatrixScreen, useMatrixVoice, type ShareAudio, type ShareQuality } from "../services/matrixVoice";
import { coverNativeVideo } from "../services/matrixScreenView";

type AudioApp = { key: string; name: string };
const CHOICE_KEY = "peligames.matrixShareAudio";
function savedChoice(): ShareAudio {
  try {
    const saved = JSON.parse(localStorage.getItem(CHOICE_KEY) || "null") as ShareAudio | null;
    if (saved?.kind === "none" || saved?.kind === "all") return saved;
    if (saved?.kind === "app" && typeof saved.key === "string" && saved.key.length <= 256) return saved;
  } catch { /* The default below. */ }
  return { kind: "all" };
}
const QUALITY_KEY = "peligames.matrixShareQuality";
const AUTO: ShareQuality = { height: 1080, fps: 30, auto: true };
function savedQuality(): ShareQuality {
  try {
    const saved = JSON.parse(localStorage.getItem(QUALITY_KEY) || "null") as Partial<ShareQuality> | null;
    if (saved && (saved.height === 720 || saved.height === 1080) && (saved.fps === 30 || saved.fps === 60)) return { height: saved.height, fps: saved.fps, auto: saved.auto === true };
  } catch { /* The default below. */ }
  return AUTO;
}
/** About how much a fixed choice sends, as shown under the buttons. */
const megabits = (quality: ShareQuality) => quality.height === 720 ? (quality.fps === 30 ? 2 : 3.5) : quality.fps === 30 ? 3.5 : 6;

export function MatrixShareButton({ icon }: { icon: React.ReactNode }) {
  const {t: tx, language} = useI18n();

  const voice = useMatrixVoice();
  const [open, setOpen] = useState(false);
  const [choice, setChoice] = useState<ShareAudio>(savedChoice);
  const [quality, setQuality] = useState<ShareQuality>(savedQuality);
  const pickQuality = (next: ShareQuality) => {
    setQuality(next);
    try { localStorage.setItem(QUALITY_KEY, JSON.stringify(next)); } catch { /* Kept for this session only. */ }
  };
  const [apps, setApps] = useState<AudioApp[] | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const connected = voice.phase === "connected";
  const loadApps = () => { setApps(null); void invoke<AudioApp[]>("matrix_screen_audio_apps").then(setApps).catch(() => setApps([])); };
  useEffect(() => {
    if (!open) return;
    loadApps();
    const uncover = coverNativeVideo();
    const outside = (event: PointerEvent) => { if (!box.current?.contains(event.target as Node)) setOpen(false); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") setOpen(false); };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => { uncover(); document.removeEventListener("pointerdown", outside); document.removeEventListener("keydown", escape); };
  }, [open]);
  const pick = (next: ShareAudio) => {
    setChoice(next);
    try { localStorage.setItem(CHOICE_KEY, JSON.stringify(next)); } catch { /* Kept for this session only. */ }
  };
  const start = () => { setOpen(false); void shareMatrixScreen(choice, quality); };
  const sizes: [ShareQuality["height"] | "auto", string][] = [["auto", tx("ui-matrix-share-button", "automatic")], [720, "720p"], [1080, "1080p"]];
  const same = (a: ShareAudio, b: ShareAudio) => a.kind === b.kind && (a.kind !== "app" || (b.kind === "app" && a.key === b.key));
  // A remembered application that is not playing now still shows, so the choice is visible.
  const listed = apps && choice.kind === "app" && !apps.some(app => app.key === choice.key) ? [...apps, { key: choice.key, name: choice.key }] : apps;
  const option = (value: ShareAudio, label: string, hint?: string) => <label key={value.kind === "app" ? `app:${value.key}` : value.kind} className={same(choice, value) ? "is-chosen" : undefined}>
    <input type="radio" name="matrix-share-audio" checked={same(choice, value)} onChange={() => pick(value)}/>
    <span>{label}{hint && <small>{hint}</small>}</span>
  </label>;
  if (voice.screen !== "off") {
    return <button type="button" className="matrix-call-share is-live" aria-pressed="true" disabled={voice.screen === "choosing"} onClick={() => void shareMatrixScreen(null)}>
      {icon}{voice.screen === "choosing" ? (tx("ui-matrix-share-button", "choosing_screen")) : (tx("ui-matrix-share-button", "stop_sharing"))}</button>;
  }
  return <div ref={box} className="matrix-share-anchor">
    <button type="button" className="matrix-call-share" aria-haspopup="dialog" aria-expanded={open} disabled={!connected} onClick={() => setOpen(value => !value)}>{icon}{tx("ui-matrix-share-button", "share_screen")}</button>
    {open && <div className="matrix-share-menu" role="dialog" aria-label={tx("ui-matrix-share-button", "stream_sound_and_quality")}>
      <strong>{tx("ui-matrix-share-button", "stream_sound")}</strong>
      {option({ kind: "all" }, tx("ui-matrix-share-button", "all_pc_sound"), tx("ui-matrix-share-button", "except_the_call_itself"))}
      {option({ kind: "none" }, tx("ui-matrix-share-button", "no_sound"))}
      <div className="matrix-share-apps-heading"><span>{tx("ui-matrix-share-button", "one_application_only")}</span><button type="button" onClick={loadApps}>{tx("ui-matrix-share-button", "refresh")}</button></div>
      <div className="matrix-share-apps">
        {listed === null ? <small>{tx("ui-matrix-share-button", "looking")}</small>
          : listed.length ? listed.map(app => option({ kind: "app", key: app.key }, app.name))
          : <small>{tx("ui-matrix-share-button", "no_application_playing_sound_now_open_the_game_and_press_refresh")}</small>}
      </div>
      <strong>{tx("ui-matrix-share-button", "quality")}</strong>
      <div className="matrix-share-quality" role="radiogroup" aria-label={tx("ui-matrix-share-button", "resolution")}>
        {sizes.map(([value, label]) => {
          const chosen = value === "auto" ? quality.auto : !quality.auto && quality.height === value;
          return <button key={value} type="button" role="radio" aria-checked={chosen} className={chosen ? "is-chosen" : undefined}
            onClick={() => pickQuality(value === "auto" ? { ...quality, auto: true } : { ...quality, height: value, auto: false })}>{label}</button>;
        })}
      </div>
      <div className="matrix-share-quality" role="radiogroup" aria-label={tx("ui-matrix-share-button", "frames_per_second")}>
        {([30, 60] as const).map(fps => {
          const chosen = !quality.auto && quality.fps === fps;
          return <button key={fps} type="button" role="radio" disabled={quality.auto} aria-checked={chosen} className={chosen ? "is-chosen" : undefined} onClick={() => pickQuality({ ...quality, fps })}>{fps} fps</button>;
        })}
      </div>
      <small>{quality.auto
        ? (tx("ui-matrix-share-button", "sends_1080p_720p_and_360p_each_viewer_gets_what_their_connection_carries_adjusted_on_the"))
        : (tx("ui-matrix-share-button", "fixed_quality", {height: quality.height, fps: quality.fps, bitrate: new Intl.NumberFormat(language).format(megabits(quality))}))}</small>
      <button type="button" className="matrix-share-start" onClick={start}>{tx("ui-matrix-share-button", "choose_screen_and_share")}</button>
      <small className="matrix-share-note">{tx("ui-matrix-share-button", "the_system_then_asks_which_screen_or_window_to_show")}</small>
    </div>}
  </div>;
}
